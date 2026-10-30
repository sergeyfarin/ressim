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
//! # Completions and crossflow
//!
//! A well has one or more completions sharing a single BHP, each with its own connected cell, well
//! index and head offset.
//!
//! **Crossflow is explicitly rejected, not clipped.** The plan permits either implementing it or
//! rejecting it as outside V1, and forbids silently clipping it. Rejection is what is implemented:
//! a producer whose connection would take fluid *from* the wellbore, or an injector whose
//! connection would draw formation fluid *into* it, returns [`WellError::Crossflow`] naming the
//! completion.
//!
//! The reason it is rejected rather than implemented is that crossflow is the one thing in this
//! model that genuinely requires a wellbore state. Without it, a producer's stream is just the sum
//! of its connections' and an injector's is prescribed, so there is nothing to mix and no wellbore
//! flash to solve. With it, the fluid re-entering the formation is the wellbore mixture, which
//! depends on every other connection — a coupled wellbore equation that would have to be posed,
//! solved and validated, and which nothing in V1's scope needs.
//!
//! A **single-completion** producer at or above its cell pressure is a different case and is shut
//! in rather than rejected: there is no other connection for fluid to have come from, so there is
//! no wellbore mixture being silently invented.
//!
//! # With water (V1b, #46)
//!
//! When the relative permeability is the water extension's, a producer also draws the water phase
//! at its own mobility, and the cell's total mobility that an injector's stream moves at includes
//! water. An injector injects either a hydrocarbon composition or water ([`InjectedFluid`]), never
//! a mixture: water is outside the EOS, so a mixed stream would need a phase split nothing here
//! defines. Injected water is at the connection pressure, so its molar density carries the BHP
//! derivative exactly as an injected hydrocarbon stream's does.

use crate::ad::Ad;
use crate::fluid::derivatives::{DerivativeError, FlashDerivatives, flash_derivatives};
use crate::fluid::flash::{FlashError, FlashState, PhaseState, flash};
use crate::fluid::specification::FluidSpecification;
use crate::fluid::transport::{TransportError, lbc_viscosity, surface_separation};
use crate::fluid::units::{PA_S_PER_CP, bar_to_pa, dpa_to_dbar};

use super::flux::hydrocarbon_kr_with_water;
use super::relperm::RelativePermeabilityModel;
use super::state::CompositionalCellState;

// Counts source evaluations in tests, so the rate-control cost claim is an assertion rather than
// a wall-clock measurement that depends on the machine.
#[cfg(test)]
thread_local! {
    static WELL_SOURCE_EVALUATIONS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn take_well_source_evaluations() -> usize {
    WELL_SOURCE_EVALUATIONS.with(|count| count.replace(0))
}

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
    /// Both hydrocarbon streams, summed. Water is not included. Rarely what anyone means, but
    /// unambiguous when it is.
    Total,
    /// Surface water, `moles * M_H2O / rho_w,surface`. Only with water.
    Water,
}

/// What an injector injects.
#[derive(Clone, Debug, PartialEq)]
pub enum InjectedFluid {
    /// A hydrocarbon stream: `N` overall mole fractions, flashed at the connection pressure.
    Hydrocarbon(Vec<f64>),
    /// Water. Only in a case with water.
    Water,
}

/// One connection between a well and a cell.
#[derive(Clone, Debug, PartialEq)]
pub struct Completion {
    /// The connected cell.
    pub cell: usize,
    /// Geometric well index [m³·cP/(day·bar)] — Peaceman **without** mobility folded in.
    pub well_index: f64,
    /// Hydrostatic head from the datum down to this completion [bar]; `p_conn = bhp + head`.
    ///
    /// Positive for a completion below the datum. With gravity enabled these differ between
    /// completions, which is what makes a multi-completion well's connections see different
    /// connection pressures from one shared BHP.
    pub head_offset_bar: f64,
}

/// A compositional well: one or more completions sharing a single BHP.
#[derive(Clone, Debug, PartialEq)]
pub struct CompositionalWell {
    /// Stable identifier for reporting.
    pub id: String,
    /// Connections, in no particular order. At least one.
    pub completions: Vec<Completion>,
    pub control: WellControl,
    /// What the well injects. Required for an injector.
    pub injection: Option<InjectedFluid>,
}

impl CompositionalWell {
    /// A single-completion well, which is what most tests and the simplest cases need.
    pub fn single(
        id: impl Into<String>,
        cell: usize,
        well_index: f64,
        control: WellControl,
    ) -> Self {
        Self {
            id: id.into(),
            completions: vec![Completion {
                cell,
                well_index,
                head_offset_bar: 0.0,
            }],
            control,
            injection: None,
        }
    }

    /// True when this well has more than one connection, and therefore a wellbore in which fluid
    /// could mix if crossflow were permitted.
    pub fn is_multi_completion(&self) -> bool {
        self.completions.len() > 1
    }
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
    /// The surface separation flash failed.
    SurfaceFlash(FlashError),
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
    /// A water injector or a water-rate control in a case without water.
    NoWaterPhase { well: String },
    /// A completion would flow the wrong way for the well's overall direction.
    ///
    /// Rejected rather than clipped: the fluid re-entering the formation would be the wellbore
    /// mixture, which V1 does not model. See the module docs.
    Crossflow {
        well: String,
        completion: usize,
        cell: usize,
        potential_bar: f64,
    },
    /// A well has no completions.
    NoCompletions { well: String },
}

impl core::fmt::Display for WellError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::CellFlash(e) => write!(f, "connected cell's flash failed: {e}"),
            Self::CellDerivative(e) => write!(f, "connected cell's derivatives failed: {e}"),
            Self::InjectionFlash(e) => write!(f, "injection stream's flash failed: {e}"),
            Self::SurfaceFlash(e) => write!(f, "surface separation flash failed: {e}"),
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
            Self::Crossflow {
                well,
                completion,
                cell,
                potential_bar,
            } => write!(
                f,
                "well {well}: completion {completion} (cell {cell}) would cross-flow at a \
                 potential of {potential_bar} bar. Crossflow is outside V1 and is rejected \
                 rather than clipped, because the fluid re-entering the formation would be the \
                 wellbore mixture, which is not modelled"
            ),
            Self::NoCompletions { well } => write!(f, "well {well} has no completions"),
            Self::NoWaterPhase { well } => {
                write!(f, "well {well} needs a water phase, and the case has none")
            }
        }
    }
}

/// A well's contribution to one cell, and its derivatives.
#[derive(Clone, Debug, PartialEq)]
pub struct WellSource {
    /// Component molar sources [mol/day]. Positive means entering the cell. With water, `N+1`
    /// entries, water last.
    pub component_moles_per_day: Vec<f64>,
    /// `d source_i / d u`, row-major `N x N`, `u = [p_bar, z_0..z_(N-2)]` of the connected cell;
    /// with water `N+1` square and `u` ends in `Sw`.
    pub cell_jacobian: Vec<Vec<f64>>,
    /// `d source_i / d bhp` [mol/day/bar], one entry per component.
    pub bhp_derivative: Vec<f64>,
    /// The BHP actually used [bar] — the target, or the limit when it bound.
    pub bhp_bar: f64,
    /// Total reservoir volumetric rate at connection conditions [m³/day], signed into the cell.
    pub reservoir_rate_m3_per_day: f64,
    /// True when a rate target was overridden by its BHP limit.
    pub on_bhp_limit: bool,
    /// The connected cell, so a caller placing sources into a grid does not have to track it.
    pub cell: usize,
    /// `p_cell - p_conn` [bar]. Positive draws from the formation.
    pub potential_bar: f64,
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

/// The rate-control equation at one BHP, kept separate from the source it constrains.
///
/// This is the row a `StandardWell`-style solve needs: `achieved(rate) - target = 0`, with the
/// BHP as its unknown and derivatives to both the BHP and the connected cells' primaries. The
/// assembly eliminates the BHP locally rather than adding a global tail to the cell system; see
/// [`super::assembly`].
#[derive(Clone, Debug, PartialEq)]
pub struct RateConstraint {
    /// `achieved - target`, in the control's own unit [mol/day] or [m³/day].
    pub residual: f64,
    /// `d residual / d bhp`, in the same unit per bar.
    pub bhp_derivative: f64,
    /// `d residual / d u` for each completion's connected cell, `u` in the cell-block layout.
    pub cell_jacobian: Vec<(usize, Vec<f64>)>,
    /// Scale used to judge the constraint residual: `|target|`, floored at one.
    pub scale: f64,
    /// The achieved rate at this BHP, in the control's unit.
    pub achieved: f64,
}

/// A well evaluated at a given BHP under its control.
#[derive(Clone, Debug, PartialEq)]
pub struct WellEvaluation {
    /// The source terms at the BHP this evaluation selected.
    pub result: WellResult,
    /// The active rate equation, or `None` when the well is on BHP control — either because that
    /// is its declared control or because its rate target binds its BHP limit.
    pub constraint: Option<RateConstraint>,
}

/// The first BHP a Newton solve tries for each well, in the caller's well order.
///
/// A BHP-controlled well starts at its target. A rate-controlled well starts halfway between its
/// BHP limit and the mean connection pressure: those are the two pressures its answer lies
/// between, so the midpoint is the least arbitrary guess that needs no history. A rejected
/// timestep therefore starts from the same place as a fresh one, which is what keeps a retry
/// reproducible.
pub fn initial_bhp_guesses(
    wells: &[CompositionalWell],
    cells: &[CompositionalCellState],
) -> Vec<f64> {
    wells
        .iter()
        .map(|well| match &well.control {
            WellControl::Bhp { target_bar } => *target_bar,
            WellControl::MolarRate {
                target_mol_per_day,
                bhp_limit_bar,
            } => rate_initial_bhp(well, cells, *target_mol_per_day, *bhp_limit_bar),
            WellControl::SurfaceRate {
                target_m3_per_day,
                bhp_limit_bar,
                ..
            } => rate_initial_bhp(well, cells, *target_m3_per_day, *bhp_limit_bar),
        })
        .collect()
}

fn rate_initial_bhp(
    well: &CompositionalWell,
    cells: &[CompositionalCellState],
    target: f64,
    bhp_limit_bar: f64,
) -> f64 {
    let (lo, hi) = rate_bhp_interval(target, bhp_limit_bar);
    if !(lo < hi) {
        return bhp_limit_bar;
    }
    let mean_connection_pressure = if well.completions.is_empty() {
        0.5 * (lo + hi)
    } else {
        well.completions
            .iter()
            .map(|completion| cells[completion.cell].pressure_bar - completion.head_offset_bar)
            .sum::<f64>()
            / well.completions.len() as f64
    };
    let midpoint = 0.5 * (bhp_limit_bar + mean_connection_pressure);
    midpoint.clamp(lo, hi)
}

/// The admissible BHP interval for a rate target: the limit is a maximum for an injector and a
/// minimum for a producer.
fn rate_bhp_interval(target: f64, bhp_limit_bar: f64) -> (f64, f64) {
    if target > 0.0 {
        (BHP_SEARCH_MIN_BAR, bhp_limit_bar.min(BHP_SEARCH_MAX_BAR))
    } else {
        (bhp_limit_bar.max(BHP_SEARCH_MIN_BAR), BHP_SEARCH_MAX_BAR)
    }
}

/// Whether a rate target is close enough to the limit's capacity that the limit should control.
///
/// This is the black-oil hysteresis rule (`well_control.rs`'s `bhp_margin`): a well stays on its
/// BHP limit until the limit can deliver the target by a margin of 5% of that target. Without the
/// margin a well sitting exactly at capacity flips control mode on every Newton iterate.
fn rate_limit_binds(achieved_at_limit: f64, target: f64) -> bool {
    let margin = 0.05 * target.abs().max(1.0);
    if target >= 0.0 {
        achieved_at_limit < target + margin
    } else {
        achieved_at_limit > target - margin
    }
}

/// Evaluate a well's source at a given BHP, ignoring its control.
///
/// Public because the rate-control solve drives it, and because a caller that wants to ask "what
/// would this well do at that pressure" should not have to construct a fake control to find out.
pub fn source_at_bhp(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    well: &CompositionalWell,
    completion: &Completion,
    cell: &CompositionalCellState,
    bhp_bar: f64,
) -> Result<WellSource, WellError> {
    if !completion.well_index.is_finite() || completion.well_index < 0.0 {
        return Err(WellError::InvalidWellIndex {
            value: completion.well_index,
        });
    }
    if matches!(well.injection, Some(InjectedFluid::Water)) && relperm.water().is_none() {
        return Err(WellError::NoWaterPhase {
            well: well.id.clone(),
        });
    }
    // Slots: the cell's block (`N`, or `N+1` with water), then BHP.
    macro_rules! sized {
        ($n:literal, $dry:literal, $wet:literal) => {
            if relperm.water().is_some() {
                source_ad::<$n, $wet>(spec, relperm, well, completion, cell, bhp_bar)
            } else {
                source_ad::<$n, $dry>(spec, relperm, well, completion, cell, bhp_bar)
            }
        };
    }
    match spec.component_count() {
        2 => sized!(2, 3, 4),
        3 => sized!(3, 4, 5),
        4 => sized!(4, 5, 6),
        5 => sized!(5, 6, 7),
        6 => sized!(6, 7, 8),
        7 => sized!(7, 8, 9),
        8 => sized!(8, 9, 10),
        9 => sized!(9, 10, 11),
        10 => sized!(10, 11, 12),
        count => Err(WellError::UnsupportedComponentCount { count }),
    }
}

/// Seed a flash-derivative row into the well's slot layout.
///
/// `N` is the component count and `M` the slot count (`N + 1`, or `N + 2` with water), so the row
/// occupies slots `0..N` and the `Sw` and BHP slots stay zero. Writing this over `M` alone is an off-by-one that indexes
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
        out[M - 1] = dpa_to_dbar(row[0]);
    }
    out
}

/// AD over the cell's block (`N` primaries, or `N+1` with water) and then `bhp`, in the last slot.
///
/// One evaluation gives both the cell block and the BHP column. Producing them separately is how a
/// BHP derivative ends up disagreeing with the cell derivative it shares every term with.
fn source_ad<const N: usize, const M: usize>(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    well: &CompositionalWell,
    completion: &Completion,
    cell: &CompositionalCellState,
    bhp_bar: f64,
) -> Result<WellSource, WellError> {
    let water = relperm.water();
    // Cell block size, and the BHP slot after it.
    let b = M - 1;
    debug_assert_eq!(b, N + usize::from(water.is_some()));
    let t = spec.reservoir_temperature_k();

    let mut p_deriv = [0.0; M];
    p_deriv[0] = 1.0;
    let p_cell = Ad::<M>::seeded(cell.pressure_bar, p_deriv);
    // The cell's water saturation in its slot, `N`; only read when there is water.
    let sw = water.map(|_| Ad::<M>::variable(cell.water_saturation, N));

    let mut bhp_deriv = [0.0; M];
    bhp_deriv[b] = 1.0;
    let bhp = Ad::<M>::seeded(bhp_bar, bhp_deriv);
    let p_conn = bhp + completion.head_offset_bar;
    let dp = p_cell - p_conn;

    let injecting = dp.value() < 0.0 && well.injection.is_some();

    let mut component_flux = vec![Ad::<M>::constant(0.0); b];
    let reservoir_rate;

    if injecting {
        // --- Injector: the prescribed stream, moving at the **perforation cell's** total
        // mobility.
        //
        // The composition and the molar density are the injected stream's: what enters the cell is
        // what the well was told to inject, evaluated at the connection pressure, and nothing
        // about the cell changes that. The *resistance* is the cell's, summed over whatever phases
        // it holds, because that is the rock the stream has to move through.
        //
        // This is the law OPM's `StandardWell` uses and C12 measured it. ResSim originally used
        // the injected stream's own mobility, on the reasoning that its saturations sum to one and
        // so the injectivity cannot vanish for the wrong reason. The total mobility of the cell
        // has that property too — it is a sum over the phases actually present — and it is what an
        // independent simulator does. The difference is not cosmetic: while the perforation cell
        // still holds decane, pure supercritical CO2 is about four times more mobile than the
        // mixture it is displacing, so injecting at the stream's own mobility over-injects by that
        // factor until the cell is swept. Against `flowexp_comp` on `1D_COMP` it showed up as a
        // 27% excess across the interval where the injector switches from rate to BHP control.
        let injection =
            well.injection
                .as_ref()
                .ok_or_else(|| WellError::MissingInjectionComposition {
                    well: well.id.clone(),
                })?;

        let z_cell = cell.overall_composition();
        let cell_state = flash(spec, bar_to_pa(cell.pressure_bar), t, &z_cell, None)
            .map_err(WellError::CellFlash)?;
        let cell_derivatives =
            flash_derivatives(spec, bar_to_pa(cell.pressure_bar), t, &z_cell, &cell_state)
                .map_err(WellError::CellDerivative)?;

        let mut lambda = Ad::<M>::constant(0.0);
        for liquid in [true, false] {
            let Some(phase) =
                cell_phase::<N, M>(spec, relperm, &cell_state, &cell_derivatives, liquid, sw)
                    .map_err(WellError::Transport)?
            else {
                continue;
            };
            lambda = lambda + phase.mobility;
        }
        if let (Some(w), Some(sw)) = (water, sw) {
            lambda = lambda + w.relperm.kr_water(sw) / w.properties.viscosity_cp(p_cell);
        }

        // Positive into the cell. The composition carries a BHP derivative and no cell-primary
        // derivative; the mobility carries cell-primary derivatives and none in BHP. Both are
        // right, and keeping them separate is what makes an injector's Jacobian auditable.
        let q = (-dp) * lambda * completion.well_index;
        reservoir_rate = q.value();
        match injection {
            InjectedFluid::Hydrocarbon(z_inj) => {
                let p_conn_pa = bar_to_pa(p_conn.value());
                let inj_state =
                    flash(spec, p_conn_pa, t, z_inj, None).map_err(WellError::InjectionFlash)?;
                let inj_derivatives = flash_derivatives(spec, p_conn_pa, t, z_inj, &inj_state)
                    .map_err(WellError::CellDerivative)?;
                let c_mixture = mixture_molar_density::<N, M>(&inj_state, &inj_derivatives);
                for (i, flux) in component_flux.iter_mut().take(N).enumerate() {
                    *flux = q * c_mixture * z_inj[i];
                }
            }
            InjectedFluid::Water => {
                let w = water.ok_or_else(|| WellError::NoWaterPhase {
                    well: well.id.clone(),
                })?;
                // At the connection pressure: the BHP derivative comes with it.
                component_flux[N] = q * w.properties.molar_density(p_conn);
            }
        }
    } else if dp.value() > 0.0 {
        // --- Producer: the cell's own fluid.
        let z = cell.overall_composition();
        let state =
            flash(spec, bar_to_pa(cell.pressure_bar), t, &z, None).map_err(WellError::CellFlash)?;
        let derivatives = flash_derivatives(spec, bar_to_pa(cell.pressure_bar), t, &z, &state)
            .map_err(WellError::CellDerivative)?;

        let mut total_rate = Ad::<M>::constant(0.0);
        for liquid in [true, false] {
            let Some(phase) = cell_phase::<N, M>(spec, relperm, &state, &derivatives, liquid, sw)
                .map_err(WellError::Transport)?
            else {
                continue;
            };
            let q = phase.mobility * dp * completion.well_index;
            total_rate = total_rate + q;
            for (i, flux) in component_flux.iter_mut().take(N).enumerate() {
                // Negative: production removes moles from the cell.
                *flux = *flux - q * phase.molar_density * phase.composition[i];
            }
        }
        if let (Some(w), Some(sw)) = (water, sw) {
            let props = &w.properties;
            let q =
                w.relperm.kr_water(sw) / props.viscosity_cp(p_cell) * dp * completion.well_index;
            total_rate = total_rate + q;
            component_flux[N] = -(q * props.molar_density(p_cell));
        }
        reservoir_rate = -total_rate.value();
    } else {
        // dp == 0, or a producer with dp < 0 and no injection composition. A single-completion
        // producer flowing backwards is not production, and there is no specified wellbore
        // composition to inject, so the rate is zero rather than something invented.
        reservoir_rate = 0.0;
    }

    let mut cell_jacobian = vec![vec![0.0; b]; b];
    let mut bhp_derivative = vec![0.0; b];
    for (i, flux) in component_flux.iter().enumerate() {
        cell_jacobian[i].copy_from_slice(&flux.deriv()[..b]);
        bhp_derivative[i] = flux.d(b);
    }

    Ok(WellSource {
        component_moles_per_day: component_flux.iter().map(|f| f.value()).collect(),
        cell_jacobian,
        bhp_derivative,
        bhp_bar,
        reservoir_rate_m3_per_day: reservoir_rate,
        on_bhp_limit: false,
        cell: completion.cell,
        potential_bar: dp.value(),
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

/// The connected cell's phase properties as AD quantities over the well's slot layout.
struct CellPhase<const M: usize> {
    mobility: Ad<M>,
    molar_density: Ad<M>,
    composition: Vec<Ad<M>>,
}

fn cell_phase<const N: usize, const M: usize>(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    state: &FlashState,
    d: &FlashDerivatives,
    liquid: bool,
    water_saturation: Option<Ad<M>>,
) -> Result<Option<CellPhase<M>>, TransportError> {
    phase_common::<N, M>(spec, relperm, state, d, liquid, false, water_saturation)
}

/// Build one phase's mobility, molar density and composition as AD quantities.
///
/// `into_bhp_slot` chooses where the derivatives land: the cell's primaries for the connected
/// cell's fluid, or the BHP slot for a stream evaluated at `p_conn`. Only the cell path is used
/// now that an injector takes its mobility from the perforation cell, but the choice stays
/// explicit rather than implied by the call site.
fn phase_common<const N: usize, const M: usize>(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    state: &FlashState,
    d: &FlashDerivatives,
    liquid: bool,
    into_bhp_slot: bool,
    water_saturation: Option<Ad<M>>,
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

    // Both curves are keyed on the **liquid** saturation, so one value serves both phases.
    let liquid_saturation = match state.phase_state {
        PhaseState::SingleLiquid => Ad::<M>::constant(1.0),
        PhaseState::SingleVapour => Ad::<M>::constant(0.0),
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
            one - s_v
        }
    };

    let mu_pa_s = lbc_viscosity::<Ad<M>>(
        spec,
        spec.reservoir_temperature_k(),
        &composition,
        molar_density,
    )?;
    let kr = match (relperm.water(), water_saturation) {
        (Some(w), Some(sw)) => hydrocarbon_kr_with_water(w, sw, liquid_saturation, liquid),
        _ => relperm.kr(liquid_saturation, liquid),
    };
    let mobility = kr / (mu_pa_s / PA_S_PER_CP);

    Ok(Some(CellPhase {
        mobility,
        molar_density,
        composition,
    }))
}

/// Largest BHP considered when solving a rate target [bar].
pub(crate) const BHP_SEARCH_MAX_BAR: f64 = 2000.0;
/// Smallest BHP considered [bar].
pub(crate) const BHP_SEARCH_MIN_BAR: f64 = 1.0;
/// Bracketing tolerance on BHP [bar].
const BHP_SOLVE_TOLERANCE_BAR: f64 = 1e-9;

/// Every completion's contribution, plus the well-level totals.
#[derive(Clone, Debug, PartialEq)]
pub struct WellResult {
    /// One entry per completion, in the well's completion order.
    pub completions: Vec<WellSource>,
    /// The BHP used [bar] — shared by every completion.
    pub bhp_bar: f64,
    /// True when a rate target was overridden by its BHP limit.
    pub on_bhp_limit: bool,
}

impl WellResult {
    /// Total component molar rate into the reservoir [mol/day], summed over completions.
    pub fn total_component_moles_per_day(&self, component_count: usize) -> Vec<f64> {
        let mut out = vec![0.0; component_count];
        for source in &self.completions {
            for (i, q) in source.component_moles_per_day.iter().enumerate() {
                out[i] += q;
            }
        }
        out
    }

    /// Total molar rate into the reservoir [mol/day]. Negative for a producer.
    pub fn total_moles_per_day(&self) -> f64 {
        self.completions
            .iter()
            .map(|s| s.total_moles_per_day())
            .sum()
    }

    pub fn is_producing(&self) -> bool {
        self.total_moles_per_day() < 0.0
    }

    /// Place this well's sources into a per-cell source table.
    ///
    /// Additive, so several wells can contribute to one table and two completions of the same well
    /// in one cell accumulate rather than overwrite.
    pub fn add_to_sources(&self, sources: &mut [Vec<f64>]) {
        for source in &self.completions {
            for (i, q) in source.component_moles_per_day.iter().enumerate() {
                sources[source.cell][i] += q;
            }
        }
    }
}

/// Evaluate a well under its control at a given BHP guess.
///
/// A BHP-controlled well ignores the guess and uses its target. A rate-controlled well first asks
/// whether its limit can meet the target; if it cannot (within the control hysteresis), the well
/// is evaluated at the limit with no rate equation. Otherwise the source is evaluated at the
/// guess and returned with the rate row that the Newton system eliminates.
///
/// This runs **one or two** source evaluations per call, never a bisection. That is the C14
/// change: the rate equation is part of the Newton system, so the BHP is a Newton unknown rather
/// than something solved inside every Jacobian evaluation.
pub fn evaluate_well_control(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    well: &CompositionalWell,
    cells: &[CompositionalCellState],
    bhp_bar: f64,
) -> Result<WellEvaluation, WellError> {
    match &well.control {
        WellControl::Bhp { target_bar } => Ok(WellEvaluation {
            result: well_source_at_bhp(spec, relperm, well, cells, *target_bar)?,
            constraint: None,
        }),
        WellControl::MolarRate {
            target_mol_per_day,
            bhp_limit_bar,
        } => evaluate_rate_control(
            spec,
            relperm,
            well,
            cells,
            *target_mol_per_day,
            *bhp_limit_bar,
            RateMeasure::Molar,
            bhp_bar,
        ),
        WellControl::SurfaceRate {
            target_m3_per_day,
            phase,
            bhp_limit_bar,
        } => evaluate_rate_control(
            spec,
            relperm,
            well,
            cells,
            *target_m3_per_day,
            *bhp_limit_bar,
            RateMeasure::Surface(*phase),
            bhp_bar,
        ),
    }
}

/// The rate equation at a BHP, with no limit check. Used by the control evaluation and by the
/// finite-difference contract test.
pub fn rate_constraint_at_bhp(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    well: &CompositionalWell,
    cells: &[CompositionalCellState],
    bhp_bar: f64,
    target: f64,
    measure: RateMeasure,
) -> Result<RateConstraint, WellError> {
    let result = well_source_at_bhp(spec, relperm, well, cells, bhp_bar)?;
    rate_constraint_from_result(spec, relperm, &well.id, &result, target, measure)
}

fn rate_constraint_from_result(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    well_id: &str,
    result: &WellResult,
    target: f64,
    measure: RateMeasure,
) -> Result<RateConstraint, WellError> {
    let block = relperm.water().is_some() as usize + spec.component_count();
    let (achieved, gradient) = measure.value_and_gradient(spec, relperm, well_id, result)?;

    let mut by_cell: Vec<(usize, Vec<f64>)> = Vec::new();
    for completion in &result.completions {
        let entry = match by_cell
            .iter_mut()
            .find(|(cell, _)| *cell == completion.cell)
        {
            Some((_, row)) => row,
            None => {
                by_cell.push((completion.cell, vec![0.0; block]));
                &mut by_cell.last_mut().expect("just pushed").1
            }
        };
        for i in 0..block {
            let weight = gradient.get(i).copied().unwrap_or(0.0);
            if weight == 0.0 {
                continue;
            }
            for (v, value) in entry.iter_mut().enumerate() {
                *value += weight * completion.cell_jacobian[i][v];
            }
        }
    }

    let mut bhp_derivative = 0.0;
    for completion in &result.completions {
        for (i, weight) in gradient.iter().enumerate() {
            bhp_derivative += weight * completion.bhp_derivative[i];
        }
    }

    Ok(RateConstraint {
        residual: achieved - target,
        bhp_derivative,
        cell_jacobian: by_cell,
        scale: target.abs().max(1.0),
        achieved,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RateMeasure {
    /// Total component molar rate [mol/day].
    Molar,
    /// Surface volumetric rate in one phase [m³/day].
    Surface(SurfacePhase),
}

impl RateMeasure {
    fn value_and_gradient(
        self,
        spec: &FluidSpecification,
        relperm: &RelativePermeabilityModel,
        well_id: &str,
        result: &WellResult,
    ) -> Result<(f64, Vec<f64>), WellError> {
        let water = relperm.water();
        let width = spec.component_count() + usize::from(water.is_some());
        match self {
            Self::Molar => {
                let components = result.total_component_moles_per_day(width);
                Ok((components.iter().sum(), vec![1.0; width]))
            }
            Self::Surface(phase) => {
                let components = result.total_component_moles_per_day(width);
                surface_measure_with_gradient(spec, relperm, well_id, &components, phase)
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_rate_control(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    well: &CompositionalWell,
    cells: &[CompositionalCellState],
    target: f64,
    bhp_limit_bar: f64,
    measure: RateMeasure,
    bhp_bar: f64,
) -> Result<WellEvaluation, WellError> {
    let (lo, hi) = rate_bhp_interval(target, bhp_limit_bar);
    let limit_bhp = if target > 0.0 { hi } else { lo };

    let mut at_limit = well_source_at_bhp(spec, relperm, well, cells, limit_bhp)?;
    let (achieved_at_limit, _) = measure.value_and_gradient(spec, relperm, &well.id, &at_limit)?;
    if lo >= hi || rate_limit_binds(achieved_at_limit, target) {
        at_limit.on_bhp_limit = true;
        for source in &mut at_limit.completions {
            source.on_bhp_limit = true;
        }
        return Ok(WellEvaluation {
            result: at_limit,
            constraint: None,
        });
    }

    let bhp_bar = bhp_bar.clamp(lo, hi);
    let result = well_source_at_bhp(spec, relperm, well, cells, bhp_bar)?;
    let constraint =
        rate_constraint_from_result(spec, relperm, &well.id, &result, target, measure)?;
    Ok(WellEvaluation {
        result,
        constraint: Some(constraint),
    })
}

/// A surface-phase rate and its gradient with respect to the **signed** component mole rates.
fn surface_measure_with_gradient(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    well_id: &str,
    components: &[f64],
    phase: SurfacePhase,
) -> Result<(f64, Vec<f64>), WellError> {
    let n = spec.component_count();
    let water = relperm.water();
    let width = n + usize::from(water.is_some());
    if components.len() != width {
        return Err(WellError::SurfaceConditionsNotPinned);
    }
    let mut gradient = vec![0.0; width];

    if phase == SurfacePhase::Water {
        let w = water.ok_or_else(|| WellError::NoWaterPhase {
            well: well_id.to_string(),
        })?;
        gradient[n] = w.properties.surface_volume_m3(1.0);
        return Ok((w.properties.surface_volume_m3(components[n]), gradient));
    }
    if spec.surface().is_none() {
        return Err(WellError::SurfaceConditionsNotPinned);
    }
    let total: f64 = components[..n].iter().sum();
    if total == 0.0 {
        return Ok((0.0, gradient));
    }
    let sign = if total < 0.0 { -1.0 } else { 1.0 };
    let stream: Vec<f64> = components[..n].iter().map(|m| sign * m).collect();
    let (volume, volume_gradient) = surface_volume_and_gradient(spec, well_id, &stream, phase)?;
    gradient[..n].copy_from_slice(&volume_gradient);
    Ok((sign * volume, gradient))
}

/// Surface volume of a non-negative component stream, and `d volume / d stream_i`.
fn surface_volume_and_gradient(
    spec: &FluidSpecification,
    well_id: &str,
    stream: &[f64],
    phase: SurfacePhase,
) -> Result<(f64, Vec<f64>), WellError> {
    let n = spec.component_count();
    let total: f64 = stream.iter().sum();
    if total == 0.0 {
        return Ok((0.0, vec![0.0; n]));
    }
    if spec.separator_stages().is_empty() {
        surface_volume_gradient_single_stage(spec, well_id, stream, phase)
    } else {
        // A liquid-cascade train chains several flashes. Its derivative is not needed by the
        // physical model, only by Newton's convergence rate, so a finite difference of the same
        // public separation is enough here and avoids duplicating the train's internals. The
        // single-stage case — every pinned fluid and every scenario — is analytic.
        let mut gradient = vec![0.0; n];
        let base = surface_volume_of_stream(spec, well_id, stream, phase)?;
        let step_scale = 1e-6 * total.max(1.0) / n as f64;
        for j in 0..n {
            let step = step_scale.max(1e-12 * stream[j].abs().max(1.0));
            let mut up = stream.to_vec();
            let mut down = stream.to_vec();
            up[j] += step;
            down[j] = (down[j] - step).max(0.0);
            let actual = up[j] - down[j];
            if actual == 0.0 {
                continue;
            }
            let upper = surface_volume_of_stream(spec, well_id, &up, phase)?;
            let lower = surface_volume_of_stream(spec, well_id, &down, phase)?;
            gradient[j] = (upper - lower) / actual;
        }
        let _ = base;
        Ok((base, gradient))
    }
}

fn surface_volume_of_stream(
    spec: &FluidSpecification,
    well_id: &str,
    stream: &[f64],
    phase: SurfacePhase,
) -> Result<f64, WellError> {
    let separated = surface_separation(spec, stream).map_err(WellError::Transport)?;
    match phase {
        SurfacePhase::Liquid => Ok(separated.liquid_volume),
        SurfacePhase::Vapour => Ok(separated.vapour_volume),
        SurfacePhase::Total => Ok(separated.liquid_volume + separated.vapour_volume),
        SurfacePhase::Water => Err(WellError::NoWaterPhase {
            well: well_id.to_string(),
        }),
    }
}

fn surface_volume_gradient_single_stage(
    spec: &FluidSpecification,
    well_id: &str,
    stream: &[f64],
    phase: SurfacePhase,
) -> Result<(f64, Vec<f64>), WellError> {
    let n = spec.component_count();
    let total: f64 = stream.iter().sum();
    let z: Vec<f64> = stream.iter().map(|m| m / total).collect();
    let surface = spec
        .surface()
        .ok_or(WellError::SurfaceConditionsNotPinned)?;
    let state = flash(spec, surface.pressure_pa, surface.temperature_k, &z, None)
        .map_err(WellError::SurfaceFlash)?;
    let derivatives =
        flash_derivatives(spec, surface.pressure_pa, surface.temperature_k, &z, &state)
            .map_err(WellError::CellDerivative)?;

    let two_phase = state.phase_state == PhaseState::TwoPhase;
    let beta = match state.phase_state {
        PhaseState::SingleLiquid => 0.0,
        PhaseState::SingleVapour => 1.0,
        PhaseState::TwoPhase => state.beta,
    };

    let dbeta_dm = |j: usize| -> f64 {
        if !two_phase || derivatives.dbeta.is_empty() {
            return 0.0;
        }
        let mut value = 0.0;
        for (k, z_k) in z.iter().enumerate().take(n - 1) {
            let dz = if j == n - 1 {
                -z_k / total
            } else if j == k {
                (1.0 - z_k) / total
            } else {
                -z_k / total
            };
            value += derivatives.dbeta[1 + k] * dz;
        }
        value
    };

    let density_gradient = |j: usize, liquid: bool| -> f64 {
        let row = if liquid {
            &derivatives.dliquid_molar_density
        } else {
            &derivatives.dvapour_molar_density
        };
        if row.is_empty() {
            return 0.0;
        }
        let mut value = 0.0;
        for (k, z_k) in z.iter().enumerate().take(n - 1) {
            let dz = if j == n - 1 {
                -z_k / total
            } else if j == k {
                (1.0 - z_k) / total
            } else {
                -z_k / total
            };
            value += row[1 + k] * dz;
        }
        value
    };

    let mut volume = 0.0;
    let mut gradient = vec![0.0; n];
    for liquid in [true, false] {
        let included = match phase {
            SurfacePhase::Liquid => liquid,
            SurfacePhase::Vapour => !liquid,
            SurfacePhase::Total => true,
            SurfacePhase::Water => false,
        };
        if !included {
            continue;
        }
        let present = matches!(
            (state.phase_state, liquid),
            (PhaseState::TwoPhase, _)
                | (PhaseState::SingleLiquid, true)
                | (PhaseState::SingleVapour, false)
        );
        if !present {
            continue;
        }
        let props = if liquid {
            state.liquid.as_ref().expect("liquid")
        } else {
            state.vapour.as_ref().expect("vapour")
        };
        let phase_moles = total * if liquid { 1.0 - beta } else { beta };
        let phase_volume = phase_moles / props.molar_density;
        volume += phase_volume;

        for (j, value) in gradient.iter_mut().enumerate() {
            let dbeta = dbeta_dm(j);
            let d_moles = if liquid {
                1.0 - beta - total * dbeta
            } else {
                beta + total * dbeta
            };
            let d_rho = density_gradient(j, liquid);
            *value += d_moles / props.molar_density
                - phase_moles * d_rho / (props.molar_density * props.molar_density);
        }
    }
    let _ = well_id;
    Ok((volume, gradient))
}

/// Every completion at a given BHP, with crossflow rejected.
///
/// The well's overall direction is decided by the sum of the completions' potentials weighted by
/// their well indices — a well whose connections mostly draw is a producer — and any completion
/// flowing against that direction is [`WellError::Crossflow`]. A single-completion well cannot
/// cross-flow against itself, so its at-or-above-cell-pressure case stays the shut-in it was.
pub fn well_source_at_bhp(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    well: &CompositionalWell,
    cells: &[CompositionalCellState],
    bhp_bar: f64,
) -> Result<WellResult, WellError> {
    #[cfg(test)]
    WELL_SOURCE_EVALUATIONS.with(|count| count.set(count.get() + 1));
    if well.completions.is_empty() {
        return Err(WellError::NoCompletions {
            well: well.id.clone(),
        });
    }

    let mut sources = Vec::with_capacity(well.completions.len());
    for completion in &well.completions {
        sources.push(source_at_bhp(
            spec,
            relperm,
            well,
            completion,
            &cells[completion.cell],
            bhp_bar,
        )?);
    }

    if well.is_multi_completion() {
        // Direction from the index-weighted potential: a completion with a larger well index has
        // more say in which way the well as a whole is flowing.
        let weighted: f64 = well
            .completions
            .iter()
            .zip(&sources)
            .map(|(c, s)| c.well_index * s.potential_bar)
            .sum();
        let producing = weighted > 0.0;

        for (index, (completion, source)) in well.completions.iter().zip(&sources).enumerate() {
            let against = if producing {
                source.potential_bar < 0.0
            } else {
                source.potential_bar > 0.0
            };
            if against {
                return Err(WellError::Crossflow {
                    well: well.id.clone(),
                    completion: index,
                    cell: completion.cell,
                    potential_bar: source.potential_bar,
                });
            }
        }
    }

    Ok(WellResult {
        completions: sources,
        bhp_bar,
        on_bhp_limit: false,
    })
}

/// Evaluate a well under its control, solving for BHP when the control is a rate.
pub fn well_source(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    well: &CompositionalWell,
    cells: &[CompositionalCellState],
) -> Result<WellResult, WellError> {
    match &well.control {
        WellControl::Bhp { target_bar } => {
            well_source_at_bhp(spec, relperm, well, cells, *target_bar)
        }
        WellControl::MolarRate {
            target_mol_per_day,
            bhp_limit_bar,
        } => solve_for_rate(
            spec,
            relperm,
            well,
            cells,
            *bhp_limit_bar,
            *target_mol_per_day,
            |result, _| Ok(result.total_moles_per_day()),
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
                cells,
                *bhp_limit_bar,
                *target_m3_per_day,
                move |result, spec| surface_rate(spec, relperm, &well.id, result, phase),
            )
        }
    }
}

/// Surface volumetric rate of a well's total stream [m³/day], signed as the stream is.
fn surface_rate(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    well_id: &str,
    result: &WellResult,
    phase: SurfacePhase,
) -> Result<f64, WellError> {
    let n = spec.component_count();
    let water = relperm.water();
    let mut components = result.total_component_moles_per_day(n + usize::from(water.is_some()));
    if phase == SurfacePhase::Water {
        let w = water.ok_or_else(|| WellError::NoWaterPhase {
            well: well_id.to_string(),
        })?;
        return Ok(w.properties.surface_volume_m3(components[n]));
    }
    if spec.surface().is_none() {
        return Err(WellError::SurfaceConditionsNotPinned);
    }
    // The hydrocarbon stream only: water does not enter the separator flash.
    components.truncate(n);
    let total: f64 = components.iter().sum();
    if total == 0.0 {
        return Ok(0.0);
    }
    // The separation works on a non-negative stream, so a producer's is negated and the sign
    // reapplied afterwards. Flashing a negative composition would be flashing nothing.
    //
    // The stream flashed here is the **sum over completions**, which is the wellbore mixture as
    // far as surface reporting is concerned. That much mixing is just addition; what V1 does not
    // model is a mixture flowing back *into* the formation, which is why crossflow is rejected.
    let sign = if total < 0.0 { -1.0 } else { 1.0 };
    let stream: Vec<f64> = components.iter().map(|m| sign * m).collect();
    let separated = surface_separation(spec, &stream).map_err(WellError::Transport)?;
    let volume = match phase {
        SurfacePhase::Liquid => separated.liquid_volume,
        SurfacePhase::Vapour => separated.vapour_volume,
        SurfacePhase::Total => separated.liquid_volume + separated.vapour_volume,
        SurfacePhase::Water => 0.0,
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
    relperm: &RelativePermeabilityModel,
    well: &CompositionalWell,
    cells: &[CompositionalCellState],
    bhp_limit_bar: f64,
    target: f64,
    measure: F,
) -> Result<WellResult, WellError>
where
    F: Fn(&WellResult, &FluidSpecification) -> Result<f64, WellError>,
{
    let injecting = target > 0.0;

    // The limit is a maximum for an injector and a minimum for a producer, so it is one end of the
    // search interval and the "hardest achievable" point.
    let (mut lo, mut hi) = rate_bhp_interval(target, bhp_limit_bar);
    if lo >= hi {
        let mut at_limit = well_source_at_bhp(spec, relperm, well, cells, bhp_limit_bar)?;
        at_limit.on_bhp_limit = true;
        return Ok(at_limit);
    }

    // At the limit the well is working as hard as it is allowed to.
    let limit_bhp = if injecting { hi } else { lo };
    let mut at_limit = well_source_at_bhp(spec, relperm, well, cells, limit_bhp)?;
    let achieved_at_limit = measure(&at_limit, spec)?;

    if rate_limit_binds(achieved_at_limit, target) {
        // The target is beyond what the limit allows: revert to BHP control at the limit and
        // report what was achieved, rather than pretending the target was met.
        at_limit.on_bhp_limit = true;
        return Ok(at_limit);
    }

    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        let result = well_source_at_bhp(spec, relperm, well, cells, mid)?;
        let value = measure(&result, spec)?;

        if hi - lo < BHP_SOLVE_TOLERANCE_BAR {
            return Ok(result);
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

    well_source_at_bhp(spec, relperm, well, cells, 0.5 * (lo + hi))
}
