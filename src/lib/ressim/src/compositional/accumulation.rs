//! Cell molar inventory, the accumulation residual and its Jacobian (compositional plan, C8).
//!
//! # The equation
//!
//! For component `i` in one cell, with pore volume `PV(p)` and the flash's phase split:
//!
//! ```text
//! n_i = PV(p) * ( S_L cL x_i + S_V cV y_i )                       [mol]
//! R_i = n_i(new) - n_i(previous) - dt * source_i                  [mol]
//! ```
//!
//! Flux between cells is C9's; this module owns the cell-local terms only. `source_i` is positive
//! for injection and negative for production, in mol/day, and `dt` is in days — the reservoir time
//! unit this crate already uses.
//!
//! **The previous inventory is evaluated once, from the previous accepted state.** It is a
//! constant with respect to the current Newton primaries and carries no derivatives. Recomputing
//! it from the trial state would make the residual a function of itself.
//!
//! # Single-phase states
//!
//! Only the phase that exists contributes. The absent phase's molar density is not multiplied by a
//! zero saturation — it is never read, because it does not exist. The plan is explicit about this,
//! and it is not pedantry: `evaluate` on an absent branch can return a root that is perfectly
//! finite and physically meaningless, and `0 * garbage` is `0` only until the garbage is a NaN.
//!
//! # Why the sum-over-phases form, when a shorter one exists
//!
//! `(1-beta) x_i + beta y_i = z_i` identically, so the inventory also equals `PV * z_i / v_mix`.
//! That form is shorter and manifestly conservative. The phase-sum form is implemented here
//! because it is the one the plan specifies and the one C9's flux has to agree with term by term,
//! and the identity is used as an **independent check** instead: a test asserts the two agree to
//! roundoff on every fixture state. Implementing the short form and testing it against itself
//! would prove nothing.

use crate::ad::Ad;
use crate::fluid::derivatives::{DerivativeError, FlashDerivatives, flash_derivatives};
use crate::fluid::flash::{FlashError, FlashState, PhaseState, flash};
use crate::fluid::specification::FluidSpecification;
use crate::fluid::units::{bar_to_pa, dpa_to_dbar};

use super::layout::MAX_COMPONENTS;
use super::state::{CompositionalCellState, RockView};

/// Why an accumulation term could not be produced.
#[derive(Clone, Debug, PartialEq)]
pub enum AccumulationError {
    /// The flash at the cell's primaries failed.
    Flash(FlashError),
    /// The flash converged but could not be differentiated.
    Derivative(DerivativeError),
    /// No AD instantiation exists for this component count.
    UnsupportedComponentCount { count: usize },
    /// The source vector is the wrong length or not finite.
    InvalidSource { reason: &'static str },
    /// A timestep that is not strictly positive.
    NonPositiveTimestep { days: f64 },
}

impl core::fmt::Display for AccumulationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Flash(e) => write!(f, "flash failed: {e}"),
            Self::Derivative(e) => write!(f, "derivatives failed: {e}"),
            Self::UnsupportedComponentCount { count } => {
                write!(f, "no AD instantiation compiled for {count} components")
            }
            Self::InvalidSource { reason } => write!(f, "invalid source: {reason}"),
            Self::NonPositiveTimestep { days } => write!(f, "dt = {days} days is not positive"),
        }
    }
}

/// A cell's molar inventory and everything derived from it.
#[derive(Clone, Debug, PartialEq)]
pub struct CellInventory {
    /// Moles of each component [mol].
    pub component_moles: Vec<f64>,
    /// Total moles [mol].
    pub total_moles: f64,
    /// Pore volume at the cell's pressure [m³].
    pub pore_volume_m3: f64,
    /// Mixture molar volume [m³/mol].
    pub mixture_molar_volume: f64,
    /// Phase state at the cell's primaries.
    pub phase_state: PhaseState,
    /// Vapour saturation — the volume fraction, not `beta`.
    pub vapour_saturation: f64,
}

/// Inventory of one cell at its current primaries.
///
/// The flash is performed here rather than taken as an argument so the inventory cannot be
/// evaluated against a phase split from different primaries.
pub fn cell_inventory(
    spec: &FluidSpecification,
    rock: &RockView<'_>,
    cell_index: usize,
    cell: &CompositionalCellState,
) -> Result<(CellInventory, FlashState), AccumulationError> {
    let z = cell.overall_composition();
    let state = flash(
        spec,
        bar_to_pa(cell.pressure_bar),
        spec.reservoir_temperature_k(),
        &z,
        None,
    )
    .map_err(AccumulationError::Flash)?;

    let pore_volume = rock.pore_volume_m3(cell_index, cell.pressure_bar);
    let inventory = inventory_from_flash(&state, &z, pore_volume);
    Ok((inventory, state))
}

/// The inventory implied by an already-computed flash.
fn inventory_from_flash(state: &FlashState, z: &[f64], pore_volume_m3: f64) -> CellInventory {
    let n = z.len();
    let v_mix = state.mixture_molar_volume();
    let total_moles = pore_volume_m3 / v_mix;

    // The sum-over-phases form, with only the phases that exist contributing.
    let mut component_moles = vec![0.0; n];
    match state.phase_state {
        PhaseState::SingleLiquid => {
            let cl = state.liquid.as_ref().expect("liquid").molar_density;
            for i in 0..n {
                component_moles[i] = pore_volume_m3 * cl * state.x[i];
            }
        }
        PhaseState::SingleVapour => {
            let cv = state.vapour.as_ref().expect("vapour").molar_density;
            for i in 0..n {
                component_moles[i] = pore_volume_m3 * cv * state.y[i];
            }
        }
        PhaseState::TwoPhase => {
            let cl = state.liquid.as_ref().expect("liquid").molar_density;
            let cv = state.vapour.as_ref().expect("vapour").molar_density;
            let s_v = state.vapour_saturation();
            let s_l = 1.0 - s_v;
            for i in 0..n {
                component_moles[i] =
                    pore_volume_m3 * (s_l * cl * state.x[i] + s_v * cv * state.y[i]);
            }
        }
    }

    CellInventory {
        component_moles,
        total_moles,
        pore_volume_m3,
        mixture_molar_volume: v_mix,
        phase_state: state.phase_state,
        vapour_saturation: state.vapour_saturation(),
    }
}

/// The accumulation residual and its local Jacobian for one cell.
#[derive(Clone, Debug, PartialEq)]
pub struct CellAccumulation {
    /// `R_i` for each component [mol]. Unscaled.
    pub residual: Vec<f64>,
    /// `dR_i/du`, row-major `N x N`, with `u = [p_bar, z_0 .. z_(N-2)]`.
    ///
    /// The pressure column is per **bar**, not per pascal: the primary is in reservoir units and
    /// the EOS boundary is where the conversion happens, exactly once.
    pub jacobian: Vec<Vec<f64>>,
    /// The inventory at the current primaries.
    pub inventory: CellInventory,
}

/// Assemble one cell's accumulation residual and Jacobian.
///
/// `previous_moles` is the inventory of the **previous accepted state**, evaluated once by the
/// caller and constant here. `source_moles_per_day` is positive for injection.
pub fn cell_accumulation(
    spec: &FluidSpecification,
    rock: &RockView<'_>,
    cell_index: usize,
    cell: &CompositionalCellState,
    previous_moles: &[f64],
    source_moles_per_day: &[f64],
    dt_days: f64,
) -> Result<CellAccumulation, AccumulationError> {
    let n = spec.component_count();
    if dt_days <= 0.0 || !dt_days.is_finite() {
        return Err(AccumulationError::NonPositiveTimestep { days: dt_days });
    }
    if previous_moles.len() != n || source_moles_per_day.len() != n {
        return Err(AccumulationError::InvalidSource {
            reason: "previous inventory or source has the wrong length",
        });
    }
    if source_moles_per_day.iter().any(|v| !v.is_finite())
        || previous_moles.iter().any(|v| !v.is_finite() || *v < 0.0)
    {
        return Err(AccumulationError::InvalidSource {
            reason: "a previous inventory or source entry is not finite, or inventory is negative",
        });
    }

    let (inventory, state) = cell_inventory(spec, rock, cell_index, cell)?;
    let z = cell.overall_composition();
    let derivatives = flash_derivatives(
        spec,
        bar_to_pa(cell.pressure_bar),
        spec.reservoir_temperature_k(),
        &z,
        &state,
    )
    .map_err(AccumulationError::Derivative)?;

    let jacobian = match n {
        2 => inventory_jacobian::<2>(rock, cell_index, cell, &state, &derivatives)?,
        3 => inventory_jacobian::<3>(rock, cell_index, cell, &state, &derivatives)?,
        4 => inventory_jacobian::<4>(rock, cell_index, cell, &state, &derivatives)?,
        count => return Err(AccumulationError::UnsupportedComponentCount { count }),
    };

    let residual = (0..n)
        .map(|i| {
            inventory.component_moles[i] - previous_moles[i] - dt_days * source_moles_per_day[i]
        })
        .collect();

    Ok(CellAccumulation {
        residual,
        jacobian,
        inventory,
    })
}

/// `dn_i/du` by carrying the flash's total derivatives through the inventory expression.
///
/// The seeds are the same two-pass trick C4 uses: every quantity the inventory depends on is
/// seeded with its **total** derivative with respect to `u`, so the arithmetic below produces
/// `dn_i/du` directly and there is no separately hand-written chain rule to get wrong.
///
/// `FlashDerivatives` is expressed per pascal in its pressure slot, and the primary here is bar,
/// so the pressure seeds are converted once, at the point they enter.
fn inventory_jacobian<const N: usize>(
    rock: &RockView<'_>,
    cell_index: usize,
    cell: &CompositionalCellState,
    state: &FlashState,
    d: &FlashDerivatives,
) -> Result<Vec<Vec<f64>>, AccumulationError> {
    debug_assert!(N <= MAX_COMPONENTS);

    // Slot 0 is pressure in bar; slots 1..N are the independent compositions.
    let seed = |per_pa_row: &[f64]| -> [f64; N] {
        let mut out = [0.0; N];
        if !per_pa_row.is_empty() {
            out[0] = dpa_to_dbar(per_pa_row[0]);
            out[1..N].copy_from_slice(&per_pa_row[1..N]);
        }
        out
    };

    // Pore volume: PV(p) = PV_ref exp(c (p - p_ref)), so dPV/dp_bar = c PV.
    let pv_value = rock.pore_volume_m3(cell_index, cell.pressure_bar);
    let mut pv_deriv = [0.0; N];
    pv_deriv[0] = rock.compressibility_per_bar * pv_value;
    let pv = Ad::<N>::seeded(pv_value, pv_deriv);

    let n = N;
    let mut jacobian = vec![vec![0.0; N]; n];

    match state.phase_state {
        PhaseState::SingleLiquid | PhaseState::SingleVapour => {
            let liquid = state.phase_state == PhaseState::SingleLiquid;
            let props = if liquid {
                state.liquid.as_ref().expect("liquid")
            } else {
                state.vapour.as_ref().expect("vapour")
            };
            let dc = if liquid {
                &d.dliquid_molar_density
            } else {
                &d.dvapour_molar_density
            };
            let dcomp = if liquid { &d.dx } else { &d.dy };

            let c = Ad::<N>::seeded(props.molar_density, seed(dc));
            for (i, row) in jacobian.iter_mut().enumerate() {
                let x_i = Ad::<N>::seeded(
                    if liquid { state.x[i] } else { state.y[i] },
                    seed(&dcomp[i]),
                );
                let n_i = pv * c * x_i;
                row.copy_from_slice(n_i.deriv());
            }
        }
        PhaseState::TwoPhase => {
            let cl_props = state.liquid.as_ref().expect("liquid");
            let cv_props = state.vapour.as_ref().expect("vapour");
            let cl = Ad::<N>::seeded(cl_props.molar_density, seed(&d.dliquid_molar_density));
            let cv = Ad::<N>::seeded(cv_props.molar_density, seed(&d.dvapour_molar_density));
            let beta = Ad::<N>::seeded(state.beta, seed(&d.dbeta));

            // v_mix = (1-beta)/cL + beta/cV, and the saturations follow from it. Building them
            // in AD rather than differentiating them by hand is what keeps the saturation
            // derivative consistent with the molar-volume one.
            let one = Ad::<N>::constant(1.0);
            let v_mix = (one - beta) / cl + beta / cv;
            let s_v = (beta / cv) / v_mix;
            let s_l = one - s_v;

            for (i, row) in jacobian.iter_mut().enumerate() {
                let x_i = Ad::<N>::seeded(state.x[i], seed(&d.dx[i]));
                let y_i = Ad::<N>::seeded(state.y[i], seed(&d.dy[i]));
                let n_i = pv * (s_l * cl * x_i + s_v * cv * y_i);
                row.copy_from_slice(n_i.deriv());
            }
        }
    }

    Ok(jacobian)
}

/// Dimensionless scaling for one cell's rows and primary variables.
///
/// Rows are divided by a reference number of moles so a residual becomes a relative error, and the
/// reference is **per component** rather than per cell. That is the point: dividing every row by
/// the cell's total moles lets a component holding 1e-6 of the material be converged to 1e-6 of
/// its own inventory while reporting 1e-12, and a trace component can then stall for many Newton
/// steps without ever showing up in a norm.
#[derive(Clone, Debug, PartialEq)]
pub struct EquationScaling {
    /// Divisor for each component row [mol].
    pub row_scale: Vec<f64>,
    /// Divisor for each primary variable: bar for pressure, dimensionless for compositions.
    pub primary_scale: Vec<f64>,
    pub diagnostics: ScalingDiagnostics,
}

/// What the scaling was built from, kept so a convergence argument can be reconstructed.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalingDiagnostics {
    /// Total moles the scaling was referenced to.
    pub reference_total_moles: f64,
    /// Which components fell back to the floor rather than their own inventory.
    pub floored: Vec<bool>,
    pub min_row_scale: f64,
    pub max_row_scale: f64,
}

/// Fraction of a cell's total moles below which a component's row scale is floored.
///
/// A component holding less than this of the cell has an inventory too small to be a meaningful
/// denominator — its relative error would be dominated by the roundoff of the components around
/// it. The floor keeps the row scaled to something physical without ever dividing by an exactly
/// zero inventory, which is what an absent component has.
pub const TRACE_COMPONENT_FLOOR: f64 = 1e-8;

impl EquationScaling {
    /// Build the scaling from a cell's previous accepted inventory.
    ///
    /// The **previous** inventory on purpose: a scale that moved with the trial state would make
    /// the convergence test a function of the iterate, so a step could "converge" by shrinking its
    /// own denominator.
    pub fn from_previous_inventory(previous_moles: &[f64], reference_pressure_bar: f64) -> Self {
        let total: f64 = previous_moles.iter().sum();
        let floor = TRACE_COMPONENT_FLOOR * total;

        let mut row_scale = Vec::with_capacity(previous_moles.len());
        let mut floored = Vec::with_capacity(previous_moles.len());
        for &moles in previous_moles {
            if moles > floor {
                row_scale.push(moles);
                floored.push(false);
            } else {
                // Includes the exactly-zero case, which is why this is a floor and not a guard.
                row_scale.push(floor.max(f64::MIN_POSITIVE));
                floored.push(true);
            }
        }

        let min_row_scale = row_scale.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_row_scale = row_scale.iter().cloned().fold(0.0, f64::max);

        // Primary scales: pressure against a declared reference, compositions against one, since
        // a mole fraction is already an O(1) quantity on a fixed interval.
        let mut primary_scale = vec![1.0; previous_moles.len()];
        primary_scale[0] = reference_pressure_bar.max(f64::MIN_POSITIVE);

        Self {
            row_scale,
            primary_scale,
            diagnostics: ScalingDiagnostics {
                reference_total_moles: total,
                floored,
                min_row_scale,
                max_row_scale,
            },
        }
    }

    /// Apply the row scaling to a residual.
    pub fn scale_residual(&self, residual: &[f64]) -> Vec<f64> {
        residual
            .iter()
            .zip(&self.row_scale)
            .map(|(r, s)| r / s)
            .collect()
    }

    /// Infinity norm of the scaled residual — the quantity a convergence test compares.
    pub fn scaled_residual_norm(&self, residual: &[f64]) -> f64 {
        self.scale_residual(residual)
            .into_iter()
            .map(f64::abs)
            .fold(0.0, f64::max)
    }
}
