//! C13's engine-side boundary: a configured compositional case, and the payloads that cross the
//! WASM edge.
//!
//! **Why this is not in `frontend.rs`.** Everything here is plain Rust over `serde`, with no
//! `wasm_bindgen` and no `JsValue`, so it is testable natively. `compositional/frontend.rs` is a
//! thin shell over it. That split is what makes the plan's "verify native/WASM results on the same
//! tiny fixtures" mean something: both paths run *this* code, and the shell has nothing in it that
//! could differ.
//!
//! # What a browser may and may not ask for
//!
//! The fluid is chosen by **name** from the pinned specifications, never described field by field.
//! A caller cannot invent critical properties, an interaction matrix or a temperature. C0 pinned
//! those against an external oracle and C6 measured the domain they are valid over; a payload that
//! could carry arbitrary EOS data would let a scenario quietly leave that domain.
//!
//! Relative permeability is **required and has no default**, for the reason recorded in
//! `docs/COMPOSITIONAL_VALIDATION.md` §6: a default would make the most consequential unsourced
//! assumption the one nobody had to type. `Linear` is accepted but marked verification-only, and
//! [`CaseConfig::validate`] reports that so a scenario-admission check can refuse it.
//!
//! Everything unsupported is rejected **before** anything is allocated, and the error names the
//! field. Silence about an unsupported control is how a run ends up meaning something other than
//! what the caller asked for.

use serde::{Deserialize, Serialize};

use super::assembly::Face;
use super::flux::Gravity;
use super::layout::CompositionalLayout;
use super::relperm::{CoreyParameters, RelativePermeabilityModel, RelativePermeabilityTable};
use super::state::{CompositionalCellState, CompositionalState, RockView};
use super::timestep::{CompositionalRun, TimestepOptions};
use super::wells::{Completion, CompositionalWell, SurfacePhase, WellControl};
use crate::fluid::flash::{PhaseState, flash};
use crate::fluid::pinned;
use crate::fluid::specification::FluidSpecification;
use crate::fluid::units::bar_to_pa;

/// The only schema this build accepts. An absent or different one is refused rather than guessed
/// at — the plan is explicit that an absent discriminator must not be read as compositional.
pub const CASE_SCHEMA: &str = "ressim-compositional-case/1";
/// The only checkpoint schema this build accepts.
pub const CHECKPOINT_SCHEMA: &str = "ressim-compositional-checkpoint/1";

/// Why a case could not be built. Every variant names the field that was wrong.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConfigError {
    /// The payload is not this schema. Carries what was found, so a version skew is diagnosable.
    UnsupportedSchema { found: String, expected: String },
    /// A named fluid this build does not have.
    UnknownFluid {
        name: String,
        supported: Vec<String>,
    },
    /// A field outside its admissible range.
    OutOfRange {
        field: String,
        value: f64,
        reason: String,
    },
    /// A geometry this build does not support. 1D only for now, and said so rather than flattened.
    UnsupportedGeometry { reason: String },
    /// A well control this build does not support.
    UnsupportedControl { well: String, control: String },
    /// The composition does not describe a mixture of the fluid's components.
    CompositionMismatch {
        field: String,
        expected: usize,
        found: usize,
    },
    /// Something deeper refused it — a relperm table, a layout, a flash. Carries its message.
    Rejected { field: String, reason: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchema { found, expected } => {
                write!(
                    f,
                    "unsupported schema {found:?}; this build accepts {expected:?}"
                )
            }
            Self::UnknownFluid { name, supported } => {
                write!(
                    f,
                    "unknown fluid {name:?}; supported: {}",
                    supported.join(", ")
                )
            }
            Self::OutOfRange {
                field,
                value,
                reason,
            } => write!(f, "{field} = {value} is out of range: {reason}"),
            Self::UnsupportedGeometry { reason } => write!(f, "unsupported geometry: {reason}"),
            Self::UnsupportedControl { well, control } => {
                write!(
                    f,
                    "well {well:?} asks for an unsupported control: {control}"
                )
            }
            Self::CompositionMismatch {
                field,
                expected,
                found,
            } => write!(
                f,
                "{field} has {found} entries, the fluid has {expected} components"
            ),
            Self::Rejected { field, reason } => write!(f, "{field} was rejected: {reason}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// The pinned fluid a case runs on, chosen by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FluidChoice {
    /// CO2 / methane / decane, the V1 ternary.
    PinnedTernary,
    /// Methane / decane, the V1 binary.
    PinnedBinary,
}

impl FluidChoice {
    fn build(self) -> Result<FluidSpecification, ConfigError> {
        let result = match self {
            Self::PinnedTernary => pinned::ternary(),
            Self::PinnedBinary => pinned::binary(),
        };
        result.map_err(|e| ConfigError::Rejected {
            field: "fluid".to_string(),
            reason: format!("{e:?}"),
        })
    }

    fn component_count(self) -> usize {
        match self {
            Self::PinnedTernary => 3,
            Self::PinnedBinary => 2,
        }
    }
}

/// A uniform 1D column. Deliberately narrow: the validated compositional cases are 1D, and a
/// geometry this build has never run is refused rather than accepted and hoped for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridConfig {
    pub cells: usize,
    pub dx_m: f64,
    pub dy_m: f64,
    pub dz_m: f64,
    pub porosity: f64,
    pub permeability_md: f64,
    /// Rock compressibility and its reference pressure. Zero compressibility is admissible.
    pub rock_reference_pressure_bar: f64,
    pub rock_compressibility_per_bar: f64,
}

/// The relative permeability model, which the caller must choose explicitly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "kebab-case")]
pub enum RelPermConfig {
    /// `kr_L = S_L`, `kr_V = S_V`. Admissible, and reported as verification-only.
    Linear,
    Corey {
        liquid_residual: f64,
        vapour_residual: f64,
        liquid_exponent: f64,
        vapour_exponent: f64,
        liquid_endpoint: f64,
        vapour_endpoint: f64,
    },
    Tabulated {
        liquid_saturation: Vec<f64>,
        kr_liquid: Vec<f64>,
        kr_vapour: Vec<f64>,
    },
}

impl RelPermConfig {
    fn build(&self) -> Result<RelativePermeabilityModel, ConfigError> {
        let rejected = |e: super::relperm::RelPermError| ConfigError::Rejected {
            field: "relperm".to_string(),
            reason: format!("{e}"),
        };
        Ok(match self {
            Self::Linear => RelativePermeabilityModel::Linear,
            Self::Corey {
                liquid_residual,
                vapour_residual,
                liquid_exponent,
                vapour_exponent,
                liquid_endpoint,
                vapour_endpoint,
            } => RelativePermeabilityModel::Corey(
                CoreyParameters::new(
                    *liquid_residual,
                    *vapour_residual,
                    *liquid_exponent,
                    *vapour_exponent,
                    *liquid_endpoint,
                    *vapour_endpoint,
                )
                .map_err(rejected)?,
            ),
            Self::Tabulated {
                liquid_saturation,
                kr_liquid,
                kr_vapour,
            } => RelativePermeabilityModel::Tabulated(
                RelativePermeabilityTable::new(
                    liquid_saturation.clone(),
                    kr_liquid.clone(),
                    kr_vapour.clone(),
                )
                .map_err(rejected)?,
            ),
        })
    }
}

/// A well control, as the payload expresses it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "control", rename_all = "kebab-case")]
pub enum WellControlConfig {
    Bhp {
        target_bar: f64,
    },
    /// Positive injects, negative produces.
    MolarRate {
        target_moles_per_day: f64,
        bhp_limit_bar: f64,
    },
    /// Positive injects, negative produces. `phase` is which surface stream the target refers to.
    SurfaceRate {
        target_m3_per_day: f64,
        phase: SurfacePhaseConfig,
        bhp_limit_bar: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SurfacePhaseConfig {
    Liquid,
    Vapour,
    Total,
}

impl From<SurfacePhaseConfig> for SurfacePhase {
    fn from(value: SurfacePhaseConfig) -> Self {
        match value {
            SurfacePhaseConfig::Liquid => SurfacePhase::Liquid,
            SurfacePhaseConfig::Vapour => SurfacePhase::Vapour,
            SurfacePhaseConfig::Total => SurfacePhase::Total,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WellConfig {
    pub id: String,
    /// Cell indices this well is connected to, and the geometric well index for each.
    pub completions: Vec<CompletionConfig>,
    #[serde(flatten)]
    pub control: WellControlConfig,
    /// Overall mole fractions of the injected stream. Required for an injector.
    pub injection_composition: Option<Vec<f64>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompletionConfig {
    pub cell: usize,
    pub well_index: f64,
    #[serde(default)]
    pub head_offset_bar: f64,
}

/// A complete case, as it crosses the boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaseConfig {
    pub schema: String,
    pub fluid: FluidChoice,
    pub grid: GridConfig,
    pub relperm: RelPermConfig,
    /// Initial pressure, uniform.
    pub initial_pressure_bar: f64,
    /// Initial overall composition, uniform. All `N` fractions, which must sum to one.
    pub initial_composition: Vec<f64>,
    #[serde(default)]
    pub wells: Vec<WellConfig>,
    /// Gravity is off unless a case asks for it; the validated 1D cases are horizontal.
    #[serde(default)]
    pub gravity_enabled: bool,
}

/// What [`CaseConfig::validate`] found that is admissible but worth saying out loud.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct CaseAdvisories {
    /// True when the relative permeability model is `Linear`, which
    /// `docs/COMPOSITIONAL_VALIDATION.md` §6 records as a **verification** assumption and not a
    /// physical model. A scenario-admission check should refuse to ship a case on it.
    pub relperm_is_verification_only: bool,
}

impl CaseConfig {
    /// Check everything that can be checked without allocating a run.
    ///
    /// Returns the advisories a caller should act on. Order matters a little: the schema is
    /// checked first, because every other message would be noise if the payload is from a
    /// different version.
    pub fn validate(&self) -> Result<CaseAdvisories, ConfigError> {
        if self.schema != CASE_SCHEMA {
            return Err(ConfigError::UnsupportedSchema {
                found: self.schema.clone(),
                expected: CASE_SCHEMA.to_string(),
            });
        }

        let n = self.fluid.component_count();
        let g = &self.grid;
        if g.cells == 0 {
            return Err(ConfigError::UnsupportedGeometry {
                reason: "a case needs at least one cell".to_string(),
            });
        }
        for (field, value) in [
            ("grid.dx_m", g.dx_m),
            ("grid.dy_m", g.dy_m),
            ("grid.dz_m", g.dz_m),
            ("grid.permeability_md", g.permeability_md),
        ] {
            if !(value.is_finite() && value > 0.0) {
                return Err(ConfigError::OutOfRange {
                    field: field.to_string(),
                    value,
                    reason: "must be finite and positive".to_string(),
                });
            }
        }
        if !(g.porosity.is_finite() && g.porosity > 0.0 && g.porosity < 1.0) {
            return Err(ConfigError::OutOfRange {
                field: "grid.porosity".to_string(),
                value: g.porosity,
                reason: "must be in (0, 1)".to_string(),
            });
        }
        if !(g.rock_compressibility_per_bar.is_finite() && g.rock_compressibility_per_bar >= 0.0) {
            return Err(ConfigError::OutOfRange {
                field: "grid.rock_compressibility_per_bar".to_string(),
                value: g.rock_compressibility_per_bar,
                reason: "must be finite and non-negative".to_string(),
            });
        }
        if !(self.initial_pressure_bar.is_finite() && self.initial_pressure_bar > 0.0) {
            return Err(ConfigError::OutOfRange {
                field: "initial_pressure_bar".to_string(),
                value: self.initial_pressure_bar,
                reason: "must be finite and positive".to_string(),
            });
        }

        check_composition("initial_composition", &self.initial_composition, n)?;

        for well in &self.wells {
            if well.completions.is_empty() {
                return Err(ConfigError::Rejected {
                    field: format!("wells[{}].completions", well.id),
                    reason: "a well needs at least one completion".to_string(),
                });
            }
            for c in &well.completions {
                if c.cell >= g.cells {
                    return Err(ConfigError::OutOfRange {
                        field: format!("wells[{}].completions.cell", well.id),
                        value: c.cell as f64,
                        reason: format!("the grid has {} cells", g.cells),
                    });
                }
                if !(c.well_index.is_finite() && c.well_index > 0.0) {
                    return Err(ConfigError::OutOfRange {
                        field: format!("wells[{}].completions.well_index", well.id),
                        value: c.well_index,
                        reason: "must be finite and positive".to_string(),
                    });
                }
            }
            if let Some(z) = &well.injection_composition {
                check_composition(&format!("wells[{}].injection_composition", well.id), z, n)?;
            }
            // A surface-rate control needs surface conditions, and the pinned fluids have them.
            // Naming it here keeps the failure at the boundary rather than inside a Newton step.
            if let WellControlConfig::SurfaceRate { .. } = well.control {
                let spec = self.fluid.build()?;
                if spec.surface().is_none() {
                    return Err(ConfigError::UnsupportedControl {
                        well: well.id.clone(),
                        control: "surface-rate, but this fluid has no surface conditions"
                            .to_string(),
                    });
                }
            }
        }

        Ok(CaseAdvisories {
            relperm_is_verification_only: matches!(self.relperm, RelPermConfig::Linear),
        })
    }
}

fn check_composition(field: &str, z: &[f64], expected: usize) -> Result<(), ConfigError> {
    if z.len() != expected {
        return Err(ConfigError::CompositionMismatch {
            field: field.to_string(),
            expected,
            found: z.len(),
        });
    }
    for (i, value) in z.iter().enumerate() {
        if !(value.is_finite() && *value >= 0.0) {
            return Err(ConfigError::OutOfRange {
                field: format!("{field}[{i}]"),
                value: *value,
                reason: "must be finite and non-negative".to_string(),
            });
        }
    }
    let total: f64 = z.iter().sum();
    if (total - 1.0).abs() > 1e-9 {
        return Err(ConfigError::OutOfRange {
            field: format!("{field} sum"),
            value: total,
            reason: "overall mole fractions must sum to one; they are never renormalized \
                     silently"
                .to_string(),
        });
    }
    Ok(())
}

/// A configured, runnable case.
///
/// It owns everything the timestep lifecycle borrows — the pore volumes in particular, which
/// [`RockView`] only borrows — so a caller holds one object and steps it.
pub struct CompositionalCase {
    config: CaseConfig,
    spec: FluidSpecification,
    layout: CompositionalLayout,
    relperm: RelativePermeabilityModel,
    pore_volumes: Vec<f64>,
    faces: Vec<Face>,
    wells: Vec<CompositionalWell>,
    sources: Vec<Vec<f64>>,
    options: TimestepOptions,
    run: CompositionalRun,
}

/// `DARCY_METRIC_FACTOR`, the same constant the black-oil path uses.
const DARCY: f64 = 8.526_988_8e-3;

impl CompositionalCase {
    /// Build a case, rejecting anything unsupported before allocating the run.
    pub fn new(config: CaseConfig) -> Result<Self, ConfigError> {
        config.validate()?;

        let spec = config.fluid.build()?;
        let n = spec.component_count();
        let cells = config.grid.cells;
        let g = &config.grid;

        let layout =
            CompositionalLayout::new(n, cells, config.wells.len(), total_completions(&config))
                .map_err(|e| ConfigError::Rejected {
                    field: "grid.cells".to_string(),
                    reason: format!("{e:?}"),
                })?;

        let relperm = config.relperm.build()?;
        let pore_volumes = vec![g.dx_m * g.dy_m * g.dz_m * g.porosity; cells];

        // A uniform column: every internal face has the same geometric transmissibility.
        let geom_t = DARCY * g.permeability_md * (g.dy_m * g.dz_m) / g.dx_m;
        let faces: Vec<Face> = (0..cells.saturating_sub(1))
            .map(|i| Face {
                cell_i: i,
                cell_j: i + 1,
                geom_t,
                // The supported geometry is a horizontal column, so gravity does nothing along it
                // even when a case asks for it. Wiring a non-trivial potential here needs a
                // vertical geometry, which this build does not accept.
                gravity: Gravity::OFF,
            })
            .collect();

        let wells = config
            .wells
            .iter()
            .map(|w| build_well(w))
            .collect::<Result<Vec<_>, _>>()?;

        let initial = CompositionalState::new(
            &layout,
            (0..cells)
                .map(|_| {
                    CompositionalCellState::new(
                        config.initial_pressure_bar,
                        config.initial_composition[..n - 1].to_vec(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| ConfigError::Rejected {
                    field: "initial_composition".to_string(),
                    reason: format!("{e:?}"),
                })?,
        )
        .map_err(|e| ConfigError::Rejected {
            field: "initial state".to_string(),
            reason: format!("{e:?}"),
        })?;

        Ok(Self {
            spec,
            layout,
            relperm,
            pore_volumes,
            faces,
            wells,
            sources: vec![vec![0.0; n]; cells],
            options: TimestepOptions::default(),
            run: CompositionalRun::new(initial),
            config,
        })
    }

    fn rock(&self) -> RockView<'_> {
        RockView {
            pore_volume_ref_m3: &self.pore_volumes,
            reference_pressure_bar: self.config.grid.rock_reference_pressure_bar,
            compressibility_per_bar: self.config.grid.rock_compressibility_per_bar,
        }
    }

    pub fn time_days(&self) -> f64 {
        self.run.time_days()
    }

    pub fn config(&self) -> &CaseConfig {
        &self.config
    }

    /// Attempt one step. The outcome is reported, never panicked: a flash that cannot resolve is
    /// something a scenario has to be able to show a user.
    pub fn step(&mut self, dt_days: f64) -> StepOutcome {
        // Built inline rather than through `self.rock()`: that borrows all of `self`, and the step
        // needs `self.run` mutably at the same time.
        let rock = RockView {
            pore_volume_ref_m3: &self.pore_volumes,
            reference_pressure_bar: self.config.grid.rock_reference_pressure_bar,
            compressibility_per_bar: self.config.grid.rock_compressibility_per_bar,
        };
        let report = self.run.step(
            &self.spec,
            &self.layout,
            &rock,
            &self.relperm,
            &self.faces,
            &self.wells,
            &self.sources,
            dt_days,
            self.options,
        );
        StepOutcome {
            accepted_dt_days: report.accepted_dt_days,
            time_days: self.run.time_days(),
            attempts: report.attempts.len(),
            newton_iterations: report
                .attempts
                .last()
                .map(|a| a.newton.iterations)
                .unwrap_or(0),
            failure: report.failure.as_ref().map(|(kind, message)| StepFailure {
                kind: format!("{kind:?}"),
                message: message.clone(),
            }),
        }
    }

    /// The accepted state, as the UI needs it.
    ///
    /// Per cell: pressure, the full overall composition, the phase state and the phase
    /// saturations. **No derivative arrays.** The plan asks for exactly that — the Jacobian blocks
    /// are large, per-cell and of no use to a chart, and copying them across the worker boundary
    /// every step is the kind of cost that is invisible until it is not.
    pub fn snapshot(&self) -> Snapshot {
        let n = self.spec.component_count();
        let cells = self.run.state().cells();
        let mut pressure = Vec::with_capacity(cells.len());
        let mut composition = vec![Vec::with_capacity(cells.len()); n];
        let mut phase_state = Vec::with_capacity(cells.len());
        let mut vapour_saturation = Vec::with_capacity(cells.len());

        for cell in cells {
            pressure.push(cell.pressure_bar);
            let z = cell.overall_composition();
            for (i, column) in composition.iter_mut().enumerate() {
                column.push(z[i]);
            }
            match flash(
                &self.spec,
                bar_to_pa(cell.pressure_bar),
                self.spec.reservoir_temperature_k(),
                &z,
                None,
            ) {
                Ok(state) => {
                    phase_state.push(
                        match state.phase_state {
                            PhaseState::TwoPhase => "two-phase",
                            PhaseState::SingleLiquid => "single-liquid",
                            PhaseState::SingleVapour => "single-vapour",
                        }
                        .to_string(),
                    );
                    vapour_saturation.push(state.vapour_saturation());
                }
                Err(_) => {
                    // A cell whose flash will not resolve is reported as such rather than given a
                    // plausible-looking saturation. C6 measured where that happens.
                    phase_state.push("unresolved".to_string());
                    vapour_saturation.push(f64::NAN);
                }
            }
        }

        Snapshot {
            time_days: self.run.time_days(),
            component_ids: (0..n).map(|i| self.spec.component(i).id.clone()).collect(),
            pressure,
            composition,
            phase_state,
            vapour_saturation,
            inventory: self.inventory(),
            cumulative_well_moles: self.run.cumulative_well_moles().to_vec(),
        }
    }

    /// Total moles of each component held by the grid. This is the conservation diagnostic: what
    /// it loses must equal what the wells produced, which `comp_rollback_*` checks in the engine.
    fn inventory(&self) -> Vec<f64> {
        let n = self.spec.component_count();
        let rock = self.rock();
        let mut total = vec![0.0; n];
        if let Ok(per_cell) = self.run.accepted_inventory(&self.spec, &rock) {
            for cell in per_cell {
                for (i, moles) in cell.iter().enumerate() {
                    total[i] += moles;
                }
            }
        }
        total
    }

    /// A versioned checkpoint. The configuration travels with the state, because a state without
    /// the grid and fluid it was computed on is not restorable into anything meaningful.
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            schema: CHECKPOINT_SCHEMA.to_string(),
            config: self.config.clone(),
            time_days: self.run.time_days(),
            pressure: self
                .run
                .state()
                .cells()
                .iter()
                .map(|c| c.pressure_bar)
                .collect(),
            composition: self
                .run
                .state()
                .cells()
                .iter()
                .map(|c| c.overall_composition())
                .collect(),
        }
    }

    /// Restore a checkpoint, refusing any schema but this build's.
    pub fn restore(checkpoint: &Checkpoint) -> Result<Self, ConfigError> {
        if checkpoint.schema != CHECKPOINT_SCHEMA {
            return Err(ConfigError::UnsupportedSchema {
                found: checkpoint.schema.clone(),
                expected: CHECKPOINT_SCHEMA.to_string(),
            });
        }
        let mut case = Self::new(checkpoint.config.clone())?;
        let n = case.spec.component_count();
        if checkpoint.pressure.len() != case.config.grid.cells
            || checkpoint.composition.len() != case.config.grid.cells
        {
            return Err(ConfigError::Rejected {
                field: "checkpoint".to_string(),
                reason: format!(
                    "carries {} cells, its own config says {}",
                    checkpoint.pressure.len(),
                    case.config.grid.cells
                ),
            });
        }
        let cells = checkpoint
            .pressure
            .iter()
            .zip(&checkpoint.composition)
            .map(|(p, z)| {
                if z.len() != n {
                    return Err(ConfigError::CompositionMismatch {
                        field: "checkpoint.composition".to_string(),
                        expected: n,
                        found: z.len(),
                    });
                }
                CompositionalCellState::new(*p, z[..n - 1].to_vec()).map_err(|e| {
                    ConfigError::Rejected {
                        field: "checkpoint.composition".to_string(),
                        reason: format!("{e:?}"),
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let state =
            CompositionalState::new(&case.layout, cells).map_err(|e| ConfigError::Rejected {
                field: "checkpoint".to_string(),
                reason: format!("{e:?}"),
            })?;
        case.run = CompositionalRun::new(state);
        case.run.set_time_days(checkpoint.time_days);
        Ok(case)
    }
}

fn total_completions(config: &CaseConfig) -> usize {
    config.wells.iter().map(|w| w.completions.len()).sum()
}

fn build_well(config: &WellConfig) -> Result<CompositionalWell, ConfigError> {
    let control = match &config.control {
        WellControlConfig::Bhp { target_bar } => WellControl::Bhp {
            target_bar: *target_bar,
        },
        WellControlConfig::MolarRate {
            target_moles_per_day,
            bhp_limit_bar,
        } => WellControl::MolarRate {
            target_mol_per_day: *target_moles_per_day,
            bhp_limit_bar: *bhp_limit_bar,
        },
        WellControlConfig::SurfaceRate {
            target_m3_per_day,
            phase,
            bhp_limit_bar,
        } => WellControl::SurfaceRate {
            target_m3_per_day: *target_m3_per_day,
            phase: (*phase).into(),
            bhp_limit_bar: *bhp_limit_bar,
        },
    };
    Ok(CompositionalWell {
        id: config.id.clone(),
        completions: config
            .completions
            .iter()
            .map(|c| Completion {
                cell: c.cell,
                well_index: c.well_index,
                head_offset_bar: c.head_offset_bar,
            })
            .collect(),
        control,
        injection_composition: config.injection_composition.clone(),
    })
}

/// What one step did.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepOutcome {
    /// `None` when no attempt succeeded; `failure` then says why.
    pub accepted_dt_days: Option<f64>,
    pub time_days: f64,
    pub attempts: usize,
    pub newton_iterations: usize,
    pub failure: Option<StepFailure>,
}

impl StepOutcome {
    pub fn succeeded(&self) -> bool {
        self.accepted_dt_days.is_some()
    }
}

/// A failure a scenario has to be able to show someone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepFailure {
    /// `Flash`, `Linear`, `Nonlinear`, `Admissibility` or `Budget`.
    pub kind: String,
    pub message: String,
}

/// The accepted state, per cell, plus the totals a conservation check needs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub time_days: f64,
    pub component_ids: Vec<String>,
    pub pressure: Vec<f64>,
    /// `composition[i][cell]` — component-major, because a chart wants one component across the
    /// grid far more often than one cell across components.
    pub composition: Vec<Vec<f64>>,
    pub phase_state: Vec<String>,
    pub vapour_saturation: Vec<f64>,
    /// Total moles per component held by the grid.
    pub inventory: Vec<f64>,
    /// Per well, cumulative component moles. Positive into the reservoir.
    pub cumulative_well_moles: Vec<Vec<f64>>,
}

/// A versioned checkpoint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub schema: String,
    pub config: CaseConfig,
    pub time_days: f64,
    pub pressure: Vec<f64>,
    pub composition: Vec<Vec<f64>>,
}
