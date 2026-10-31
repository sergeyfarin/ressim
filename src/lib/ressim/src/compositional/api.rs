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
//! Water (V1b, #46) is opt-in: a `water` block and a `stone-ii` relative permeability go
//! together, and either without the other is rejected. Without them a case is exactly the
//! water-free model it was.
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
use super::water::{StoneTwoRelPerm, WaterModel, WaterProperties};
use super::wells::{Completion, CompositionalWell, InjectedFluid, SurfacePhase, WellControl};
use crate::fluid::flash::{PhaseState, flash};
use crate::fluid::pinned;
use crate::fluid::specification::FluidSpecification;
use crate::fluid::units::bar_to_pa;
use crate::relperm::{SgofRow, SwofRow};

/// The schema for a case with **no** schedule. See [`SCHEDULE_CASE_SCHEMA`] for the scheduled
/// variant, and why the version is part of the schedule guard.
pub const CASE_SCHEMA: &str = "ressim-compositional-case/1";
/// The schema for a case that carries a schedule (#50).
///
/// **Why a second version rather than an additive field.** Serde defaults a missing `schedule` and
/// ignores an unknown one, so a build that predates schedules would accept a `/1` payload with
/// events and silently run it without them — a different physical case with no error. A scheduled
/// case therefore demands `/2`, which an older build refuses as an unsupported schema. The
/// checkpoint's embedded `config` carries the same discriminator, so a scheduled checkpoint is
/// refused by that build too, even though the checkpoint envelope itself remains `/1`.
pub const SCHEDULE_CASE_SCHEMA: &str = "ressim-compositional-case/2";
/// The only checkpoint envelope this build accepts.
///
/// Additive optional fields do not bump this: serde defaults a missing field and ignores an
/// unknown one, so a `/1` payload from before `well_bhp_bar` or `schedule_position` restores
/// unchanged. A bump is for a shape an older build would *misinterpret*, not one it can safely
/// ignore.
///
/// **Scheduled checkpoints are guarded through their embedded config schema.** A checkpoint from a
/// scheduled case carries `config.schema == SCHEDULE_CASE_SCHEMA`, which a build without schedules
/// refuses in `CompositionalCase::new`; the envelope itself stays `/1` because the extra fields are
/// additive.
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

/// A uniform 1D column unless [`Self::box_grid`] is present. Deliberately narrow: a geometry this
/// build has not been validated on is refused rather than accepted and hoped for.
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
    /// Optional `nx x ny x nz` structured geometry (#47). Absent means the uniform 1D column
    /// above, bit for bit. When present, `cells` must equal `nx * ny * nz`, `dx_m`/`dy_m` are the
    /// lateral cell sizes, and `dz_m`/`permeability_md` are the uniform fallbacks for any layer
    /// vector left empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub box_grid: Option<BoxGridConfig>,
}

/// A structured `nx x ny x nz` box with per-layer thickness and directional permeability.
///
/// Layer vectors are either empty (use the [`GridConfig`] fallback) or exactly `nz` long. They are
/// not broadcast from a single value, because a silent kz = kx default is exactly the kind of
/// unsourced rock assumption this engine refuses to make.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoxGridConfig {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    /// One thickness per layer [m], top to bottom. Empty means `GridConfig::dz_m`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layer_dz_m: Vec<f64>,
    /// One x-permeability per layer [mD]. Empty means `GridConfig::permeability_md`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layer_kx_md: Vec<f64>,
    /// One y-permeability per layer [mD]. Empty means `GridConfig::permeability_md`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layer_ky_md: Vec<f64>,
    /// One z-permeability per layer [mD]. Empty means `GridConfig::permeability_md`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layer_kz_md: Vec<f64>,
    /// Depth of the top of layer 0 [m], positive downward. Only used when gravity is enabled.
    #[serde(default)]
    pub top_depth_m: f64,
}

impl BoxGridConfig {
    /// One value per layer, with the uniform fallback applied where the vector is empty.
    fn per_layer(values: &[f64], fallback: f64, nz: usize) -> Vec<f64> {
        if values.is_empty() {
            vec![fallback; nz]
        } else {
            values.to_vec()
        }
    }

    fn layer_dz_m(&self, fallback: f64) -> Vec<f64> {
        Self::per_layer(&self.layer_dz_m, fallback, self.nz)
    }

    fn layer_kx_md(&self, fallback: f64) -> Vec<f64> {
        Self::per_layer(&self.layer_kx_md, fallback, self.nz)
    }

    fn layer_ky_md(&self, fallback: f64) -> Vec<f64> {
        Self::per_layer(&self.layer_ky_md, fallback, self.nz)
    }

    fn layer_kz_md(&self, fallback: f64) -> Vec<f64> {
        Self::per_layer(&self.layer_kz_md, fallback, self.nz)
    }

    /// Centre depth of layer `k`, positive downward from the top of layer 0.
    fn layer_depth_m(&self, k: usize, dz_m: &[f64]) -> f64 {
        let mut depth = self.top_depth_m;
        for thickness in &dz_m[..k] {
            depth += thickness;
        }
        depth + 0.5 * dz_m[k]
    }
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
    /// Three-phase Stone II over SWOF/SGOF tables. Requires, and is required by, a `water` block.
    StoneIi {
        swof: Vec<SwofRow>,
        sgof: Vec<SgofRow>,
    },
}

impl RelPermConfig {
    /// The two-phase hydrocarbon model. The three-phase one is built with the water it needs by
    /// [`CaseConfig::relperm_model`].
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
            Self::StoneIi { .. } => {
                return Err(ConfigError::Rejected {
                    field: "relperm".to_string(),
                    reason: "stone-ii is the water extension's model and needs a water block"
                        .to_string(),
                });
            }
        })
    }
}

/// The immiscible water phase: a `PVTW` record, a surface density and the initial saturation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaterConfig {
    pub reference_pressure_bar: f64,
    pub formation_volume_factor: f64,
    pub compressibility_per_bar: f64,
    pub viscosity_cp: f64,
    #[serde(default)]
    pub viscosibility_per_bar: f64,
    pub surface_density_kg_per_m3: f64,
    /// Initial water saturation, uniform, in `[0, 1)`.
    pub initial_saturation: f64,
}

impl WaterConfig {
    fn properties(&self) -> Result<WaterProperties, ConfigError> {
        WaterProperties::new(
            self.reference_pressure_bar,
            self.formation_volume_factor,
            self.compressibility_per_bar,
            self.viscosity_cp,
            self.viscosibility_per_bar,
            self.surface_density_kg_per_m3,
        )
        .map_err(|e| ConfigError::Rejected {
            field: "water".to_string(),
            reason: format!("{e}"),
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
    /// No flow through the well. Its connections stay configured, so a later event can open it
    /// again without repeating the completion data.
    ShutIn,
}

/// What a scheduled event changes about a well's injected stream.
///
/// [`Self::NoInjection`] is not "produce instead": it clears the prescribed stream, which is what
/// a well that was injecting needs before it can be reopened as a producer. A control change and
/// an injection change may appear in one event, which is how a WAG switch changes the target and
/// the fluid together without an instant at which the well is half-switched.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "injection", rename_all = "kebab-case")]
pub enum WellInjectionConfig {
    Hydrocarbon {
        composition: Vec<f64>,
    },
    Water,
    #[serde(rename = "none")]
    NoInjection,
}

impl WellInjectionConfig {
    fn to_injected(&self) -> Option<InjectedFluid> {
        match self {
            Self::Hydrocarbon { composition } => {
                Some(InjectedFluid::Hydrocarbon(composition.clone()))
            }
            Self::Water => Some(InjectedFluid::Water),
            Self::NoInjection => None,
        }
    }
}

/// One scheduled well change, in the shared declarative envelope ([`crate::schedule`]).
pub type WellEventConfig = crate::schedule::WellEvent<WellChangeConfig>;

/// The engine-specific half of a [`WellEventConfig`]: a control, an injected stream, or both.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WellChangeConfig {
    /// Replace the well's control. An event with none leaves the current control in force.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<WellControlConfig>,
    /// Replace the prescribed injected stream. An event with none leaves it in force.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub injection: Option<WellInjectionConfig>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SurfacePhaseConfig {
    Liquid,
    Vapour,
    Total,
    /// Surface water. Only in a case with water.
    Water,
}

impl From<SurfacePhaseConfig> for SurfacePhase {
    fn from(value: SurfacePhaseConfig) -> Self {
        match value {
            SurfacePhaseConfig::Liquid => SurfacePhase::Liquid,
            SurfacePhaseConfig::Vapour => SurfacePhase::Vapour,
            SurfacePhaseConfig::Total => SurfacePhase::Total,
            SurfacePhaseConfig::Water => SurfacePhase::Water,
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
    /// Overall mole fractions of the injected hydrocarbon stream. An injector needs this or
    /// `inject_water`, not both.
    pub injection_composition: Option<Vec<f64>>,
    /// Inject water. Only in a case with water.
    #[serde(default)]
    pub inject_water: bool,
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
    /// Absolute-time changes to well controls and injected streams (#50). Base [`Self::wells`]
    /// controls apply until the first event. Events are ordered by time, never sorted silently.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schedule: Vec<WellEventConfig>,
    /// Gravity is off unless a case asks for it. It acts on the vertical faces of a
    /// [`GridConfig::box_grid`]; the uniform 1D column remains horizontal whatever this says.
    #[serde(default)]
    pub gravity_enabled: bool,
    /// Initial pressure gradient [bar/m], positive downward, measured from
    /// [`BoxGridConfig::top_depth_m`]. Absent means the uniform `initial_pressure_bar` every
    /// validated 1D case uses. Requires a box grid; a gradient on a uniform column would silently
    /// do nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_pressure_gradient_bar_per_m: Option<f64>,
    /// The immiscible water phase. Absent means no water, and then `relperm` must be a two-phase
    /// model; present means `relperm` must be `stone-ii`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water: Option<WaterConfig>,
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
        if self.schema != CASE_SCHEMA && self.schema != SCHEDULE_CASE_SCHEMA {
            return Err(ConfigError::UnsupportedSchema {
                found: self.schema.clone(),
                expected: format!("{CASE_SCHEMA} or {SCHEDULE_CASE_SCHEMA}"),
            });
        }
        if !self.schedule.is_empty() && self.schema != SCHEDULE_CASE_SCHEMA {
            return Err(ConfigError::UnsupportedSchema {
                found: format!("{} (with a schedule)", self.schema),
                expected: SCHEDULE_CASE_SCHEMA.to_string(),
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
        if let Some(box_grid) = &g.box_grid {
            if box_grid.nx == 0 || box_grid.ny == 0 || box_grid.nz == 0 {
                return Err(ConfigError::Rejected {
                    field: "grid.box_grid".to_string(),
                    reason: "nx, ny and nz must each be at least one".to_string(),
                });
            }
            let expected = box_grid
                .nx
                .checked_mul(box_grid.ny)
                .and_then(|n| n.checked_mul(box_grid.nz));
            if expected != Some(g.cells) {
                return Err(ConfigError::Rejected {
                    field: "grid.box_grid".to_string(),
                    reason: format!(
                        "{} x {} x {} does not multiply to grid.cells = {}",
                        box_grid.nx, box_grid.ny, box_grid.nz, g.cells
                    ),
                });
            }
            for (name, values) in [
                ("layer_dz_m", &box_grid.layer_dz_m),
                ("layer_kx_md", &box_grid.layer_kx_md),
                ("layer_ky_md", &box_grid.layer_ky_md),
                ("layer_kz_md", &box_grid.layer_kz_md),
            ] {
                if !values.is_empty() && values.len() != box_grid.nz {
                    return Err(ConfigError::Rejected {
                        field: format!("grid.box_grid.{name}"),
                        reason: format!(
                            "must be empty (use the grid fallback) or have one value per layer \
                             ({}), found {}",
                            box_grid.nz,
                            values.len()
                        ),
                    });
                }
                for (k, value) in values.iter().enumerate() {
                    if !(value.is_finite() && *value > 0.0) {
                        return Err(ConfigError::OutOfRange {
                            field: format!("grid.box_grid.{name}[{k}]"),
                            value: *value,
                            reason: "must be finite and positive".to_string(),
                        });
                    }
                }
            }
            if !box_grid.top_depth_m.is_finite() {
                return Err(ConfigError::OutOfRange {
                    field: "grid.box_grid.top_depth_m".to_string(),
                    value: box_grid.top_depth_m,
                    reason: "must be finite".to_string(),
                });
            }
        } else if self.initial_pressure_gradient_bar_per_m.is_some() {
            return Err(ConfigError::Rejected {
                field: "initial_pressure_gradient_bar_per_m".to_string(),
                reason: "requires a grid.box_grid; the uniform 1D column is horizontal".to_string(),
            });
        }
        if let Some(gradient) = self.initial_pressure_gradient_bar_per_m
            && !gradient.is_finite()
        {
            return Err(ConfigError::OutOfRange {
                field: "initial_pressure_gradient_bar_per_m".to_string(),
                value: gradient,
                reason: "must be finite".to_string(),
            });
        }
        if !(self.initial_pressure_bar.is_finite() && self.initial_pressure_bar > 0.0) {
            return Err(ConfigError::OutOfRange {
                field: "initial_pressure_bar".to_string(),
                value: self.initial_pressure_bar,
                reason: "must be finite and positive".to_string(),
            });
        }
        if let (Some(box_grid), Some(gradient)) =
            (&g.box_grid, self.initial_pressure_gradient_bar_per_m)
        {
            let dz = box_grid.layer_dz_m(g.dz_m);
            for k in 0..box_grid.nz {
                let pressure = self.initial_pressure_bar
                    + gradient * (box_grid.layer_depth_m(k, &dz) - box_grid.top_depth_m);
                if !(pressure.is_finite() && pressure > 0.0) {
                    return Err(ConfigError::OutOfRange {
                        field: "initial_pressure_gradient_bar_per_m".to_string(),
                        value: gradient,
                        reason: format!(
                            "puts layer {k} at {pressure} bar; every cell needs a positive \
                             initial pressure"
                        ),
                    });
                }
            }
        }

        check_composition("initial_composition", &self.initial_composition, n)?;

        // Water and the three-phase relative permeability come together or not at all.
        let stone = matches!(self.relperm, RelPermConfig::StoneIi { .. });
        match (&self.water, stone) {
            (Some(water), true) => {
                if !(water.initial_saturation.is_finite()
                    && (0.0..1.0).contains(&water.initial_saturation))
                {
                    return Err(ConfigError::OutOfRange {
                        field: "water.initial_saturation".to_string(),
                        value: water.initial_saturation,
                        reason: "must be in [0, 1); a cell with no hydrocarbon is outside the \
                                 model"
                            .to_string(),
                    });
                }
                self.relperm_model()?;
            }
            (None, false) => {}
            (Some(_), false) => {
                return Err(ConfigError::Rejected {
                    field: "relperm".to_string(),
                    reason: "a case with water needs the three-phase stone-ii model".to_string(),
                });
            }
            (None, true) => {
                return Err(ConfigError::Rejected {
                    field: "water".to_string(),
                    reason: "the stone-ii model needs a water block".to_string(),
                });
            }
        }

        for (index, well) in self.wells.iter().enumerate() {
            if well.id.trim().is_empty() {
                return Err(ConfigError::Rejected {
                    field: format!("wells[{index}].id"),
                    reason: "a well needs a non-empty identifier".to_string(),
                });
            }
            if self.wells[..index].iter().any(|other| other.id == well.id) {
                return Err(ConfigError::Rejected {
                    field: format!("wells[{index}].id"),
                    reason: format!(
                        "the identifier {:?} is already used; a schedule addresses a well by it",
                        well.id
                    ),
                });
            }
        }

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
            if well.inject_water && well.injection_composition.is_some() {
                return Err(ConfigError::Rejected {
                    field: format!("wells[{}].inject_water", well.id),
                    reason: "a well injects water or a hydrocarbon composition, not both"
                        .to_string(),
                });
            }
            let wants_water = well.inject_water
                || matches!(
                    well.control,
                    WellControlConfig::SurfaceRate {
                        phase: SurfacePhaseConfig::Water,
                        ..
                    }
                );
            if wants_water && self.water.is_none() {
                return Err(ConfigError::Rejected {
                    field: format!("wells[{}]", well.id),
                    reason: "injects or meters water, and the case has no water block".to_string(),
                });
            }
            self.validate_well_control(&format!("wells[{}].control", well.id), well)?;
        }

        for (index, event) in self.schedule.iter().enumerate() {
            let field = format!("schedule[{index}]");
            let well = self
                .wells
                .iter()
                .find(|well| well.id == event.well)
                .ok_or_else(|| ConfigError::Rejected {
                    field: field.clone(),
                    reason: format!("no configured well is called {:?}", event.well),
                })?;
            if event.change.control.is_none() && event.change.injection.is_none() {
                return Err(ConfigError::Rejected {
                    field,
                    reason: "an event must change at least one of control or injection".to_string(),
                });
            }
            if let Some(control) = &event.change.control {
                let mut changed = well.clone();
                changed.control = control.clone();
                self.validate_well_control(&format!("{field}.control"), &changed)?;
            }
            if let Some(injection) = &event.change.injection {
                self.validate_well_injection(&format!("{field}.injection"), injection, n)?;
            }
        }
        crate::schedule::validate_event_times(&self.schedule).map_err(|reason| {
            ConfigError::Rejected {
                field: "schedule".to_string(),
                reason,
            }
        })?;

        Ok(CaseAdvisories {
            relperm_is_verification_only: matches!(self.relperm, RelPermConfig::Linear),
        })
    }

    /// The relative permeability model, carrying the water model when the case has water.
    fn relperm_model(&self) -> Result<RelativePermeabilityModel, ConfigError> {
        match (&self.relperm, &self.water) {
            (RelPermConfig::StoneIi { swof, sgof }, Some(water)) => {
                let relperm = StoneTwoRelPerm::new(swof.clone(), sgof.clone()).map_err(|e| {
                    ConfigError::Rejected {
                        field: "relperm".to_string(),
                        reason: format!("{e}"),
                    }
                })?;
                Ok(RelativePermeabilityModel::StoneTwoWater(Box::new(
                    WaterModel {
                        properties: water.properties()?,
                        relperm,
                    },
                )))
            }
            (relperm, _) => relperm.build(),
        }
    }

    /// Check a control in the context of the well that will run it.
    ///
    /// Base controls and scheduled controls go through the same function, so an event cannot
    /// introduce a target the base payload would have been refused. `well` supplies the identity
    /// and injection side, which a surface-water control depends on.
    fn validate_well_control(&self, field: &str, well: &WellConfig) -> Result<(), ConfigError> {
        let positive = |value: f64, field: &str| -> Result<(), ConfigError> {
            if value.is_finite() && value > 0.0 {
                Ok(())
            } else {
                Err(ConfigError::OutOfRange {
                    field: field.to_string(),
                    value,
                    reason: "must be finite and positive".to_string(),
                })
            }
        };
        let finite = |value: f64, field: &str| -> Result<(), ConfigError> {
            if value.is_finite() {
                Ok(())
            } else {
                Err(ConfigError::OutOfRange {
                    field: field.to_string(),
                    value,
                    reason: "must be finite".to_string(),
                })
            }
        };
        match &well.control {
            WellControlConfig::Bhp { target_bar } => {
                positive(*target_bar, &format!("{field}.target_bar"))?
            }
            WellControlConfig::MolarRate {
                target_moles_per_day,
                bhp_limit_bar,
            } => {
                finite(
                    *target_moles_per_day,
                    &format!("{field}.target_moles_per_day"),
                )?;
                positive(*bhp_limit_bar, &format!("{field}.bhp_limit_bar"))?;
            }
            WellControlConfig::SurfaceRate {
                target_m3_per_day,
                phase,
                bhp_limit_bar,
            } => {
                finite(*target_m3_per_day, &format!("{field}.target_m3_per_day"))?;
                positive(*bhp_limit_bar, &format!("{field}.bhp_limit_bar"))?;
                if *phase == SurfacePhaseConfig::Water && self.water.is_none() {
                    return Err(ConfigError::Rejected {
                        field: format!("{field}.phase"),
                        reason: "meters water, and the case has no water block".to_string(),
                    });
                }
                let spec = self.fluid.build()?;
                if spec.surface().is_none() {
                    return Err(ConfigError::UnsupportedControl {
                        well: well.id.clone(),
                        control: "surface-rate, but this fluid has no surface conditions"
                            .to_string(),
                    });
                }
            }
            WellControlConfig::ShutIn => {}
        }
        Ok(())
    }

    /// Check an injected stream the same way a base well's injection is checked.
    fn validate_well_injection(
        &self,
        field: &str,
        injection: &WellInjectionConfig,
        components: usize,
    ) -> Result<(), ConfigError> {
        match injection {
            WellInjectionConfig::Hydrocarbon { composition } => {
                check_composition(field, composition, components)
            }
            WellInjectionConfig::Water if self.water.is_none() => Err(ConfigError::Rejected {
                field: field.to_string(),
                reason: "injects water, and the case has no water block".to_string(),
            }),
            WellInjectionConfig::Water | WellInjectionConfig::NoInjection => Ok(()),
        }
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
    /// How many schedule events have been applied. The schedule itself lives in `config`, because
    /// a checkpoint already carries it; this is the cursor into it.
    schedule_position: usize,
    /// True when the accepted state sits exactly on an event that was applied after the step.
    ///
    /// The next call is the report remainder after that event. It may legitimately be shorter than
    /// the minimum timestep, and unlike an interval ending at an event it is not capped. This flag
    /// grants the allowance for that one remainder only; a caller's tiny step at any other time is
    /// still refused. It is not checkpointed: restore derives it by replaying to the clock.
    schedule_boundary_pending: bool,
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

        let mut layout =
            CompositionalLayout::new(n, cells, config.wells.len(), total_completions(&config))
                .map_err(|e| ConfigError::Rejected {
                    field: "grid.cells".to_string(),
                    reason: format!("{e:?}"),
                })?;
        if config.water.is_some() {
            layout = layout.with_water();
        }
        let initial_sw = config.water.as_ref().map_or(0.0, |w| w.initial_saturation);

        let relperm = config.relperm_model()?;
        let (pore_volumes, faces, initial_pressures) = match &g.box_grid {
            // The validated 1D column, byte for byte as it was before structured grids existed.
            None => {
                let geom_t = DARCY * g.permeability_md * (g.dy_m * g.dz_m) / g.dx_m;
                let faces: Vec<Face> = (0..cells.saturating_sub(1))
                    .map(|i| Face {
                        cell_i: i,
                        cell_j: i + 1,
                        geom_t,
                        // A horizontal column, so gravity does nothing along it even when a case
                        // asks for it. A vertical geometry needs `box_grid`, which can carry a
                        // depth for every face.
                        gravity: Gravity::OFF,
                    })
                    .collect();
                (
                    vec![g.dx_m * g.dy_m * g.dz_m * g.porosity; cells],
                    faces,
                    vec![config.initial_pressure_bar; cells],
                )
            }
            Some(box_grid) => {
                let dz = box_grid.layer_dz_m(g.dz_m);
                let kx = box_grid.layer_kx_md(g.permeability_md);
                let ky = box_grid.layer_ky_md(g.permeability_md);
                let kz = box_grid.layer_kz_md(g.permeability_md);
                let gradient = config.initial_pressure_gradient_bar_per_m.unwrap_or(0.0);
                let depth = |k: usize| box_grid.layer_depth_m(k, &dz);
                let index = |i: usize, j: usize, k: usize| (k * box_grid.ny + j) * box_grid.nx + i;

                let mut pore_volumes = Vec::with_capacity(cells);
                let mut initial_pressures = Vec::with_capacity(cells);
                for (k, &layer_dz) in dz.iter().enumerate() {
                    let pressure =
                        config.initial_pressure_bar + gradient * (depth(k) - box_grid.top_depth_m);
                    for _ in 0..box_grid.nx * box_grid.ny {
                        pore_volumes.push(g.dx_m * g.dy_m * layer_dz * g.porosity);
                        initial_pressures.push(pressure);
                    }
                }

                let mut faces =
                    Vec::with_capacity(cells.saturating_sub(1) * 3 + box_grid.nx * box_grid.ny);
                for k in 0..box_grid.nz {
                    for j in 0..box_grid.ny {
                        for i in 0..box_grid.nx {
                            let here = index(i, j, k);
                            if i + 1 < box_grid.nx {
                                faces.push(Face {
                                    cell_i: here,
                                    cell_j: index(i + 1, j, k),
                                    geom_t: DARCY * kx[k] * (g.dy_m * dz[k]) / g.dx_m,
                                    gravity: Gravity::OFF,
                                });
                            }
                            if j + 1 < box_grid.ny {
                                faces.push(Face {
                                    cell_i: here,
                                    cell_j: index(i, j + 1, k),
                                    geom_t: DARCY * ky[k] * (g.dx_m * dz[k]) / g.dy_m,
                                    gravity: Gravity::OFF,
                                });
                            }
                            if k + 1 < box_grid.nz {
                                let k_above = kz[k];
                                let k_below = kz[k + 1];
                                faces.push(Face {
                                    cell_i: here,
                                    cell_j: index(i, j, k + 1),
                                    // TPFA harmonic mean of the two half-cell
                                    // transmissibilities, with different thicknesses and
                                    // permeabilities across the layer boundary.
                                    geom_t: DARCY * 2.0 * k_above * k_below * g.dx_m * g.dy_m
                                        / (k_below * dz[k] + k_above * dz[k + 1]),
                                    gravity: if config.gravity_enabled {
                                        Gravity::between(depth(k), depth(k + 1))
                                    } else {
                                        Gravity::OFF
                                    },
                                });
                            }
                        }
                    }
                }
                (pore_volumes, faces, initial_pressures)
            }
        };

        let wells = config
            .wells
            .iter()
            .map(build_well)
            .collect::<Result<Vec<_>, _>>()?;

        let initial = CompositionalState::new(
            &layout,
            initial_pressures
                .iter()
                .map(|&pressure| {
                    CompositionalCellState::with_water(
                        pressure,
                        config.initial_composition[..n - 1].to_vec(),
                        initial_sw,
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

        let mut case = Self {
            spec,
            layout,
            relperm,
            pore_volumes,
            faces,
            wells,
            sources: vec![vec![0.0; layout.cell_block_size()]; cells],
            options: TimestepOptions::default(),
            run: CompositionalRun::new(initial),
            schedule_position: 0,
            schedule_boundary_pending: false,
            config,
        };
        // A t = 0 event is the initial condition, not a step-1 surprise.
        case.apply_schedule_through(0.0);
        Ok(case)
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

    /// Grid dimensions as `[nx, ny, nz]`. A uniform 1D column is `[cells, 1, 1]`.
    pub fn dimensions(&self) -> [usize; 3] {
        match &self.config.grid.box_grid {
            Some(box_grid) => [box_grid.nx, box_grid.ny, box_grid.nz],
            None => [self.config.grid.cells, 1, 1],
        }
    }

    #[cfg(test)]
    pub(crate) fn faces(&self) -> &[Face] {
        &self.faces
    }

    #[cfg(test)]
    pub(crate) fn pore_volumes(&self) -> &[f64] {
        &self.pore_volumes
    }

    pub fn config(&self) -> &CaseConfig {
        &self.config
    }

    /// Apply every schedule event whose time has been reached, in configuration order.
    ///
    /// Called at construction for `t = 0`, at the start of a step (which also makes a restored
    /// run self-consistent), and after an accepted step reaches an event boundary. Applying an
    /// event twice is harmless because every event replaces state rather than toggling it; that
    /// idempotence is what lets restore replay the prefix without storing a mutation log.
    fn apply_schedule_through(&mut self, time_days: f64) {
        let mut applied = false;
        while let Some(event) = self.config.schedule.get(self.schedule_position) {
            if !crate::schedule::event_reached(time_days, event.time_days) {
                break;
            }
            apply_well_event(&mut self.wells, event);
            self.schedule_position += 1;
            applied = true;
        }
        if applied && self.run.time_days() > 0.0 {
            self.schedule_boundary_pending = true;
        }
        if applied && self.run.well_bhp_bar().len() == self.wells.len() {
            // The accepted BHP belongs to the interval that just ended. A well the new event
            // shuts in has no flowing pressure from this instant, so report zero rather than the
            // pressure it was producing at. A reopened well keeps zero until its first accepted
            // step supplies a solved value.
            let mut bhp = self.run.well_bhp_bar().to_vec();
            for (value, well) in bhp.iter_mut().zip(&self.wells) {
                if matches!(well.control, WellControl::ShutIn) {
                    *value = 0.0;
                }
            }
            self.run.set_well_bhp_bar(bhp);
        }
    }

    /// Attempt one step. The outcome is reported, never panicked: a flash that cannot resolve is
    /// something a scenario has to be able to show a user.
    ///
    /// The requested `dt` is capped at the next schedule event, so a switch is never applied
    /// part-way through an accepted step. The caller sees the smaller accepted `dt` and continues
    /// on the remainder; [`super::frontend`] and the worker session already do this.
    ///
    /// A boundary interval may be shorter than the normal minimum timestep. That happens in both
    /// directions: an event at day `1.0000005` leaves `5e-7 d` before it, and an event at day
    /// `0.9999995` leaves `5e-7 d` of a daily report interval *after* it. Both intervals are
    /// explicitly requested by the schedule, so they lower the retry ladder's floor; refusing
    /// either would end the run with `Budget` rather than stepping. A caller's own tiny `dt` away
    /// from a schedule boundary is still refused by the configured minimum.
    pub fn step(&mut self, dt_days: f64) -> StepOutcome {
        let requested_dt_days = dt_days;
        self.apply_schedule_through(self.run.time_days());
        let starts_at_event = self.schedule_boundary_pending;
        let dt_days = crate::schedule::cap_dt_at_next_event(
            &self.config.schedule,
            self.schedule_position,
            self.run.time_days(),
            dt_days,
        );
        let mut options = self.options;
        let schedule_created_interval = dt_days < requested_dt_days || starts_at_event;
        if schedule_created_interval && dt_days > 0.0 {
            options.min_dt_days = options.min_dt_days.min(dt_days);
        }
        let next_event = self
            .config
            .schedule
            .get(self.schedule_position)
            .map(|event| event.time_days);
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
            options,
        );
        if report.succeeded() {
            self.schedule_boundary_pending = false;
            if next_event
                .is_some_and(|time| crate::schedule::event_reached(self.run.time_days(), time))
            {
                self.apply_schedule_through(self.run.time_days());
            }
        }
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
        let has_water = self.relperm.water().is_some();
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
                    // Of the pore volume: the hydrocarbons hold `1 - Sw` of it.
                    vapour_saturation
                        .push((1.0 - cell.water_saturation) * state.vapour_saturation());
                }
                Err(_) => {
                    // A cell whose flash will not resolve is reported as such rather than given a
                    // plausible-looking saturation. C6 measured where that happens.
                    phase_state.push("unresolved".to_string());
                    vapour_saturation.push(f64::NAN);
                }
            }
        }

        let mut inventory = self.inventory();
        let water = has_water.then(|| WaterSnapshot {
            saturation: cells.iter().map(|c| c.water_saturation).collect(),
            inventory_moles: inventory.pop().unwrap_or(0.0),
        });
        Snapshot {
            time_days: self.run.time_days(),
            component_ids: (0..n).map(|i| self.spec.component(i).id.clone()).collect(),
            pressure,
            composition,
            phase_state,
            vapour_saturation,
            inventory,
            cumulative_well_moles: self.run.cumulative_well_moles().to_vec(),
            well_bhp_bar: self.run.well_bhp_bar().to_vec(),
            water,
        }
    }

    /// Total moles of each component held by the grid, water last when the case has it. This is
    /// the conservation diagnostic: what it loses must equal what the wells produced, which
    /// `comp_rollback_*` checks in the engine.
    fn inventory(&self) -> Vec<f64> {
        let rock = self.rock();
        let water = self.relperm.water().map(|w| &w.properties);
        let mut total = vec![0.0; self.layout.cell_block_size()];
        if let Ok(per_cell) = self
            .run
            .accepted_inventory_with_water(&self.spec, &rock, water)
        {
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
            water_saturation: self.relperm.water().map(|_| {
                self.run
                    .state()
                    .cells()
                    .iter()
                    .map(|c| c.water_saturation)
                    .collect()
            }),
            well_bhp_bar: self.run.well_bhp_bar().to_vec(),
            schedule_position: Some(self.schedule_position),
            schedule_boundary_pending: Some(self.schedule_boundary_pending),
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
        let has_water = case.relperm.water().is_some();
        let water_saturation = match (&checkpoint.water_saturation, has_water) {
            (Some(sw), true) if sw.len() == checkpoint.pressure.len() => sw.clone(),
            (None, false) => vec![0.0; checkpoint.pressure.len()],
            _ => {
                return Err(ConfigError::Rejected {
                    field: "checkpoint.water_saturation".to_string(),
                    reason: "must be present, one per cell, exactly when the case has water"
                        .to_string(),
                });
            }
        };
        let cells = checkpoint
            .pressure
            .iter()
            .zip(&checkpoint.composition)
            .zip(&water_saturation)
            .map(|((p, z), sw)| {
                if z.len() != n {
                    return Err(ConfigError::CompositionMismatch {
                        field: "checkpoint.composition".to_string(),
                        expected: n,
                        found: z.len(),
                    });
                }
                CompositionalCellState::with_water(*p, z[..n - 1].to_vec(), *sw).map_err(|e| {
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
        // Replay the schedule to the checkpoint's clock, then require the explicit position to
        // agree. Replaying is idempotent because events replace state; the comparison is what
        // catches a checkpoint whose cursor and clock disagree.
        case.apply_schedule_through(checkpoint.time_days);
        // A checkpoint carries whether the next step is the remainder after an event. That is
        // exact; inferring it from the clock would regenerate an allowance already consumed, and
        // at day 1000 a 5e-7 d remainder is smaller than the clock tolerance. Older checkpoints
        // without the field are inferred from an event that coincides with their clock.
        case.schedule_boundary_pending =
            checkpoint.schedule_boundary_pending.unwrap_or_else(|| {
                checkpoint.time_days > 0.0
                    && case
                        .schedule_position
                        .checked_sub(1)
                        .and_then(|index| case.config.schedule.get(index))
                        .is_some_and(|event| {
                            crate::schedule::event_at(checkpoint.time_days, event.time_days)
                        })
            });
        if let Some(position) = checkpoint.schedule_position
            && position != case.schedule_position
        {
            return Err(ConfigError::Rejected {
                field: "checkpoint.schedule_position".to_string(),
                reason: format!(
                    "says {position} event(s) were applied, but the clock at {} day reaches {}",
                    checkpoint.time_days, case.schedule_position
                ),
            });
        }
        if !checkpoint.well_bhp_bar.is_empty() {
            if checkpoint.well_bhp_bar.len() != case.config.wells.len() {
                return Err(ConfigError::Rejected {
                    field: "checkpoint.well_bhp_bar".to_string(),
                    reason: format!(
                        "carries {} values for {} wells",
                        checkpoint.well_bhp_bar.len(),
                        case.config.wells.len()
                    ),
                });
            }
            for (index, bhp) in checkpoint.well_bhp_bar.iter().enumerate() {
                // Zero is a shut-in well, or a well just reopened by an event that has not taken a
                // step yet. Both are valid at an event boundary; only a negative or non-finite
                // value is not a pressure.
                if !bhp.is_finite() || *bhp < 0.0 {
                    return Err(ConfigError::Rejected {
                        field: format!("checkpoint.well_bhp_bar[{index}]"),
                        reason: "every reported BHP must be finite and non-negative; zero means \
                                 no accepted flow"
                            .to_string(),
                    });
                }
            }
            case.run.set_well_bhp_bar(checkpoint.well_bhp_bar.clone());
        }
        Ok(case)
    }
}

fn total_completions(config: &CaseConfig) -> usize {
    config.wells.iter().map(|w| w.completions.len()).sum()
}

fn build_well(config: &WellConfig) -> Result<CompositionalWell, ConfigError> {
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
        control: control_from_config(&config.control),
        injection: if config.inject_water {
            Some(InjectedFluid::Water)
        } else {
            config
                .injection_composition
                .clone()
                .map(InjectedFluid::Hydrocarbon)
        },
    })
}

/// Apply one already-validated event to the matching well.
///
/// A missing well cannot happen after [`CaseConfig::validate`], and is skipped rather than
/// panicking so a future caller that bypasses validation still gets a typed outcome instead of a
/// crash. Events replace controls and streams; nothing accumulates.
fn apply_well_event(wells: &mut [CompositionalWell], event: &WellEventConfig) {
    let Some(well) = wells.iter_mut().find(|well| well.id == event.well) else {
        return;
    };
    if let Some(control) = &event.change.control {
        well.control = control_from_config(control);
    }
    if let Some(injection) = &event.change.injection {
        well.injection = injection.to_injected();
    }
}

fn control_from_config(control: &WellControlConfig) -> WellControl {
    match control {
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
        WellControlConfig::ShutIn => WellControl::ShutIn,
    }
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
    /// Total moles per hydrocarbon component held by the grid. Water is in [`Self::water`].
    pub inventory: Vec<f64>,
    /// Per well, cumulative component moles. Positive into the reservoir. With water, each row
    /// has one more entry, water last.
    pub cumulative_well_moles: Vec<Vec<f64>>,
    /// Per well, the BHP the accepted solve ran at [bar], or zero while the well is shut in (and
    /// for a well just reopened by an event, until its first accepted step). Empty before the
    /// first accepted step and in snapshots from before C14's reporting.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub well_bhp_bar: Vec<f64>,
    /// The water phase, when the case has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water: Option<WaterSnapshot>,
}

/// The water phase's share of a snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaterSnapshot {
    /// Water saturation per cell.
    pub saturation: Vec<f64>,
    /// Total water held by the grid [mol].
    pub inventory_moles: f64,
}

/// A versioned checkpoint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub schema: String,
    pub config: CaseConfig,
    pub time_days: f64,
    pub pressure: Vec<f64>,
    pub composition: Vec<Vec<f64>>,
    /// Water saturation per cell, exactly when the case has water.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water_saturation: Option<Vec<f64>>,
    /// Per well, the accepted flowing BHP [bar]. Absent in checkpoints written before it existed;
    /// restoring one without it re-solves the BHP at the next step. Zero means shut in, or newly
    /// reopened before its first accepted step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub well_bhp_bar: Vec<f64>,
    /// How many scheduled events had been applied at this state (#50).
    ///
    /// An event replaces a well's control or stream, so the position is not derivable from the
    /// state alone when two events at the same time target different wells. It is carried
    /// explicitly; a checkpoint written before this field existed is replayed by time instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_position: Option<usize>,
    /// Whether the next step is the report remainder after an event (#50).
    ///
    /// Without this, restore would have to infer it from the clock, and a remainder already
    /// consumed could look like clock roundoff at long times. Carrying the flag keeps a restored
    /// run's timestep policy identical to the run that wrote the checkpoint. Absent in older
    /// checkpoints, which are inferred from `event_at(checkpoint.time, last event)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_boundary_pending: Option<bool>,
}
