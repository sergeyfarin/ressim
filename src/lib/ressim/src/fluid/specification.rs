//! Validated fluid specification for the compositional model (compositional plan, C1).
//!
//! A [`FluidSpecification`] is the only way to hand component data to the EOS. It is validated on
//! construction and immutable afterwards, so every later task can assume finite positive critical
//! properties, a symmetric interaction matrix and a consistent component order without rechecking.
//!
//! The pinned V1 fluids are in [`super::pinned`]. Everything here is general enough to hold them
//! and deliberately no more general than that: one EOS variant, one viscosity model, one fluid
//! region, fixed temperature.
//!
//! **Units are SI, without exception.** Pa, K, kg/mol, m³/kmol for critical volume (the unit
//! `LBC.hpp` consumes). Conversion from reservoir units happens in [`super::units`] and nowhere
//! else. See `docs/COMPOSITIONAL_VALIDATION.md` §2.

use serde::{Deserialize, Serialize};

/// Everything that can be wrong with a fluid specification or an overall composition.
///
/// Every variant names the offending field and, where the problem is component-local, its index
/// and identifier. A message that says only "invalid input" costs more to debug than the check
/// saved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FluidSpecError {
    /// Fewer than 2 or more than [`MAX_COMPONENTS`] components.
    UnsupportedComponentCount { count: usize },
    /// Two components share an identifier, so results could not be attributed to either.
    DuplicateComponentId {
        id: String,
        first: usize,
        second: usize,
    },
    /// A component identifier is empty.
    EmptyComponentId { index: usize },
    /// A critical property, molar mass or acentric factor is non-finite or out of range.
    InvalidComponentProperty {
        index: usize,
        id: String,
        field: &'static str,
        value: f64,
        reason: &'static str,
    },
    /// The interaction matrix is not `n x n`.
    InteractionMatrixShape {
        expected: usize,
        rows: usize,
        cols: Option<usize>,
    },
    /// `k_ij != k_ji`.
    InteractionMatrixAsymmetric {
        i: usize,
        j: usize,
        k_ij: f64,
        k_ji: f64,
    },
    /// An interaction coefficient is non-finite.
    InteractionCoefficientNotFinite { i: usize, j: usize, value: f64 },
    /// Reservoir or surface temperature is not a positive absolute temperature.
    InvalidTemperature { field: &'static str, value_k: f64 },
    /// Surface pressure is not positive and finite.
    InvalidSurfacePressure { value_pa: f64 },
    /// An overall composition has the wrong number of entries.
    CompositionLength { expected: usize, actual: usize },
    /// An overall composition entry is non-finite or negative.
    CompositionEntryInvalid { index: usize, value: f64 },
    /// An overall composition does not sum to one within the permitted tolerance.
    CompositionSum { sum: f64, tolerance: f64 },
    /// Every component is zero, so there is no material to flash.
    CompositionAllZero,
    /// A permutation is not a bijection of `0..n`.
    InvalidPermutation { order: Vec<usize>, count: usize },
}

impl core::fmt::Display for FluidSpecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedComponentCount { count } => write!(
                f,
                "component count {count} is outside the supported range 2..={MAX_COMPONENTS}"
            ),
            Self::DuplicateComponentId { id, first, second } => {
                write!(
                    f,
                    "duplicate component id '{id}' at indices {first} and {second}"
                )
            }
            Self::EmptyComponentId { index } => write!(f, "component {index} has an empty id"),
            Self::InvalidComponentProperty {
                index,
                id,
                field,
                value,
                reason,
            } => write!(
                f,
                "component {index} ('{id}') field {field} = {value} is invalid: {reason}"
            ),
            Self::InteractionMatrixShape {
                expected,
                rows,
                cols,
            } => match cols {
                Some(c) => write!(
                    f,
                    "interaction matrix must be {expected}x{expected}, got {rows}x{c}"
                ),
                None => write!(
                    f,
                    "interaction matrix must be {expected}x{expected}, got {rows} rows"
                ),
            },
            Self::InteractionMatrixAsymmetric { i, j, k_ij, k_ji } => write!(
                f,
                "interaction matrix is asymmetric: k[{i}][{j}] = {k_ij} but k[{j}][{i}] = {k_ji}"
            ),
            Self::InteractionCoefficientNotFinite { i, j, value } => {
                write!(
                    f,
                    "interaction coefficient k[{i}][{j}] = {value} is not finite"
                )
            }
            Self::InvalidTemperature { field, value_k } => write!(
                f,
                "{field} = {value_k} K is not a finite positive absolute temperature"
            ),
            Self::InvalidSurfacePressure { value_pa } => {
                write!(
                    f,
                    "surface pressure {value_pa} Pa is not finite and positive"
                )
            }
            Self::CompositionLength { expected, actual } => {
                write!(f, "composition has {actual} entries, expected {expected}")
            }
            Self::CompositionEntryInvalid { index, value } => {
                write!(
                    f,
                    "composition entry {index} = {value} is not finite and non-negative"
                )
            }
            Self::CompositionSum { sum, tolerance } => write!(
                f,
                "composition sums to {sum}, which is outside 1 +/- {tolerance}"
            ),
            Self::CompositionAllZero => write!(f, "composition is zero in every component"),
            Self::InvalidPermutation { order, count } => {
                write!(f, "{order:?} is not a permutation of 0..{count}")
            }
        }
    }
}

/// Upper bound on component count for V1.
///
/// Not a physical limit — a scope limit. The plan delivers N=2 and N=3 and requires an N=4
/// layout-only test to expose hidden three-component assumptions, so the type permits 4 while the
/// pinned fluids stop at 3.
pub const MAX_COMPONENTS: usize = 4;

/// Default tolerance on `sum(z) - 1`.
///
/// Loose enough to absorb the rounding of a composition that was written out to a text format and
/// read back, tight enough that a genuinely wrong composition is rejected. A caller who wants a
/// different tolerance passes it explicitly to [`OverallComposition::new_with_tolerance`].
pub const DEFAULT_COMPOSITION_SUM_TOLERANCE: f64 = 1.0e-10;

/// The equation of state.
///
/// One variant on purpose. V1 pins unmodified Peng–Robinson, which is what
/// `opm/compositional/ptflash_fixtures.json` was generated with. Because this is a tagged enum,
/// a serialized specification carrying any other variant fails to deserialize rather than being
/// silently reinterpreted — which is the actual protection wanted, and the reason it is an enum
/// rather than an implicit assumption.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EosVariant {
    /// Peng–Robinson with `f(w) = 0.37464 + 1.54226 w - 0.26992 w^2`, `Omega_A = 0.457235529`,
    /// `Omega_B = 0.077796074`, `m1, m2 = 1 +/- sqrt(2)`.
    PengRobinson,
}

/// The viscosity correlation.
///
/// One variant, for the same reason as [`EosVariant`]. Lohrenz–Bray–Clark is what the C0 fixture
/// carries, and the plan forbids an undeclared constant-viscosity approximation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViscosityModel {
    /// Lohrenz, Bray & Clark, JPT 16.10 (1964), with the paper's `-0.40758` typo corrected to
    /// `-0.040758` as OPM's `LBC.hpp` notes.
    LohrenzBrayClark,
}

/// A single component's EOS properties, all SI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Component {
    /// Stable identifier used in reporting and to key fixtures. Not a display name.
    pub id: String,
    /// Molar mass [kg/mol].
    pub molar_mass_kg_per_mol: f64,
    /// Critical temperature [K].
    pub critical_temperature_k: f64,
    /// Critical pressure [Pa].
    pub critical_pressure_pa: f64,
    /// Critical volume [m³/kmol].
    ///
    /// kmol, not mol. This is the unit `LBC.hpp` consumes — it divides by 1000 to reach m³/mol —
    /// and the C0 fixture reports it under that name. Storing it in mol here would silently
    /// break viscosity by three orders of magnitude.
    pub critical_volume_m3_per_kmol: f64,
    /// Acentric factor [-].
    pub acentric_factor: f64,
}

/// Surface conditions for the single-stage surface flash.
///
/// **Not yet pinned for V1.** `docs/COMPOSITIONAL_VALIDATION.md` §6 records this as an open
/// decision owed by C5, which is why [`FluidSpecification::surface`] is an `Option` rather than a
/// field with a plausible default. A default here would become a silent assumption in every
/// surface rate the model ever reports.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfaceConditions {
    pub pressure_pa: f64,
    pub temperature_k: f64,
}

/// A validated fluid specification. Construct with [`FluidSpecification::new`].
///
/// Fields are private so the invariants established at construction cannot be broken afterwards;
/// read them through the accessors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FluidSpecification {
    components: Vec<Component>,
    /// Row-major `n x n` binary interaction coefficients, symmetric with zero diagonal.
    interaction: Vec<Vec<f64>>,
    eos: EosVariant,
    viscosity: ViscosityModel,
    /// The single fixed reservoir temperature [K]. V1 is isothermal.
    reservoir_temperature_k: f64,
    surface: Option<SurfaceConditions>,
}

impl FluidSpecification {
    /// Validate and construct.
    ///
    /// Every check is a rejection, never a repair: nothing here normalizes, clamps or symmetrizes
    /// a value the caller got wrong. A specification that needed fixing is a specification whose
    /// provenance is unclear, and the plan forbids silently correcting it.
    pub fn new(
        components: Vec<Component>,
        interaction: Vec<Vec<f64>>,
        eos: EosVariant,
        viscosity: ViscosityModel,
        reservoir_temperature_k: f64,
        surface: Option<SurfaceConditions>,
    ) -> Result<Self, FluidSpecError> {
        let n = components.len();
        if !(2..=MAX_COMPONENTS).contains(&n) {
            return Err(FluidSpecError::UnsupportedComponentCount { count: n });
        }

        for (index, c) in components.iter().enumerate() {
            if c.id.is_empty() {
                return Err(FluidSpecError::EmptyComponentId { index });
            }
            let check = |field: &'static str, value: f64, positive: bool| {
                if !value.is_finite() {
                    Err(FluidSpecError::InvalidComponentProperty {
                        index,
                        id: c.id.clone(),
                        field,
                        value,
                        reason: "not finite",
                    })
                } else if positive && value <= 0.0 {
                    Err(FluidSpecError::InvalidComponentProperty {
                        index,
                        id: c.id.clone(),
                        field,
                        value,
                        reason: "must be strictly positive",
                    })
                } else {
                    Ok(())
                }
            };
            check("molar_mass_kg_per_mol", c.molar_mass_kg_per_mol, true)?;
            check("critical_temperature_k", c.critical_temperature_k, true)?;
            check("critical_pressure_pa", c.critical_pressure_pa, true)?;
            check(
                "critical_volume_m3_per_kmol",
                c.critical_volume_m3_per_kmol,
                true,
            )?;
            // The acentric factor of hydrogen is negative, so only finiteness is universal here.
            // A model-specific range belongs to whatever correlation consumes it, per plan C1.1.
            check("acentric_factor", c.acentric_factor, false)?;
        }

        for i in 0..n {
            for j in (i + 1)..n {
                if components[i].id == components[j].id {
                    return Err(FluidSpecError::DuplicateComponentId {
                        id: components[i].id.clone(),
                        first: i,
                        second: j,
                    });
                }
            }
        }

        if interaction.len() != n {
            return Err(FluidSpecError::InteractionMatrixShape {
                expected: n,
                rows: interaction.len(),
                cols: None,
            });
        }
        for row in &interaction {
            if row.len() != n {
                return Err(FluidSpecError::InteractionMatrixShape {
                    expected: n,
                    rows: interaction.len(),
                    cols: Some(row.len()),
                });
            }
        }
        for i in 0..n {
            for j in 0..n {
                let k_ij = interaction[i][j];
                if !k_ij.is_finite() {
                    return Err(FluidSpecError::InteractionCoefficientNotFinite {
                        i,
                        j,
                        value: k_ij,
                    });
                }
                let k_ji = interaction[j][i];
                if k_ij != k_ji {
                    return Err(FluidSpecError::InteractionMatrixAsymmetric { i, j, k_ij, k_ji });
                }
            }
        }

        if !reservoir_temperature_k.is_finite() || reservoir_temperature_k <= 0.0 {
            return Err(FluidSpecError::InvalidTemperature {
                field: "reservoir_temperature_k",
                value_k: reservoir_temperature_k,
            });
        }
        if let Some(s) = surface {
            if !s.temperature_k.is_finite() || s.temperature_k <= 0.0 {
                return Err(FluidSpecError::InvalidTemperature {
                    field: "surface_temperature_k",
                    value_k: s.temperature_k,
                });
            }
            if !s.pressure_pa.is_finite() || s.pressure_pa <= 0.0 {
                return Err(FluidSpecError::InvalidSurfacePressure {
                    value_pa: s.pressure_pa,
                });
            }
        }

        Ok(Self {
            components,
            interaction,
            eos,
            viscosity,
            reservoir_temperature_k,
            surface,
        })
    }

    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    pub fn components(&self) -> &[Component] {
        &self.components
    }

    pub fn component(&self, index: usize) -> &Component {
        &self.components[index]
    }

    /// Binary interaction coefficient `k_ij`.
    pub fn interaction(&self, i: usize, j: usize) -> f64 {
        self.interaction[i][j]
    }

    pub fn eos(&self) -> EosVariant {
        self.eos
    }

    pub fn viscosity_model(&self) -> ViscosityModel {
        self.viscosity
    }

    pub fn reservoir_temperature_k(&self) -> f64 {
        self.reservoir_temperature_k
    }

    /// Surface conditions, if they have been pinned. See [`SurfaceConditions`].
    pub fn surface(&self) -> Option<SurfaceConditions> {
        self.surface
    }

    /// Index of a component by its stable identifier.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.components.iter().position(|c| c.id == id)
    }

    /// Reorder components, carrying the interaction matrix with them.
    ///
    /// This exists so that reordering is *possible but explicit*. There is no setter that changes
    /// the component list alone: permuting components while leaving `k_ij` indexed by the old
    /// order silently pairs each component with another's interaction coefficients, and the result
    /// is a plausible-looking fluid that is not the one anybody specified. Making the permutation
    /// an operation on the whole specification is what makes that unrepresentable.
    ///
    /// `order[new_index] = old_index`.
    pub fn permuted(&self, order: &[usize]) -> Result<Self, FluidSpecError> {
        let n = self.components.len();
        let invalid = || FluidSpecError::InvalidPermutation {
            order: order.to_vec(),
            count: n,
        };
        if order.len() != n {
            return Err(invalid());
        }
        let mut seen = vec![false; n];
        for &o in order {
            if o >= n || seen[o] {
                return Err(invalid());
            }
            seen[o] = true;
        }

        let components = order.iter().map(|&o| self.components[o].clone()).collect();
        let interaction = order
            .iter()
            .map(|&oi| order.iter().map(|&oj| self.interaction[oi][oj]).collect())
            .collect();

        Self::new(
            components,
            interaction,
            self.eos,
            self.viscosity,
            self.reservoir_temperature_k,
            self.surface,
        )
    }
}

/// How a composition treats components that are exactly zero.
///
/// The distinction matters because the two are physically different and the plan forbids
/// conflating them: a *trace* component is present and must stay in the equilibrium calculation,
/// while an *absent* one carries no material. Neither may be quietly given material to make a
/// solver's life easier, and an absent component must be allowed to become present when a well
/// injects it — so this is a property of one composition, never a permanent property of the fluid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentActivity {
    /// `z_i == 0.0` exactly. No material.
    Absent,
    /// `z_i > 0.0`. Present, however small.
    Present,
}

/// A validated overall composition, in mole fractions, matching some [`FluidSpecification`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OverallComposition {
    z: Vec<f64>,
}

impl OverallComposition {
    /// Validate a composition against a specification using [`DEFAULT_COMPOSITION_SUM_TOLERANCE`].
    pub fn new(spec: &FluidSpecification, z: Vec<f64>) -> Result<Self, FluidSpecError> {
        Self::new_with_tolerance(spec, z, DEFAULT_COMPOSITION_SUM_TOLERANCE)
    }

    /// Validate with an explicit sum tolerance.
    ///
    /// The entries are stored exactly as given. Nothing is renormalized: a composition that is
    /// within tolerance is already good enough for the EOS, and one that is not is an error, not
    /// something to repair. Scaling the entries would also destroy an exact zero, which is the
    /// one property [`ComponentActivity`] depends on.
    pub fn new_with_tolerance(
        spec: &FluidSpecification,
        z: Vec<f64>,
        sum_tolerance: f64,
    ) -> Result<Self, FluidSpecError> {
        let n = spec.component_count();
        if z.len() != n {
            return Err(FluidSpecError::CompositionLength {
                expected: n,
                actual: z.len(),
            });
        }
        for (index, &value) in z.iter().enumerate() {
            if !value.is_finite() || value < 0.0 {
                return Err(FluidSpecError::CompositionEntryInvalid { index, value });
            }
        }
        let sum: f64 = z.iter().sum();
        if sum == 0.0 {
            return Err(FluidSpecError::CompositionAllZero);
        }
        if (sum - 1.0).abs() > sum_tolerance {
            return Err(FluidSpecError::CompositionSum {
                sum,
                tolerance: sum_tolerance,
            });
        }
        Ok(Self { z })
    }

    pub fn as_slice(&self) -> &[f64] {
        &self.z
    }

    pub fn len(&self) -> usize {
        self.z.len()
    }

    pub fn is_empty(&self) -> bool {
        self.z.is_empty()
    }

    /// Activity of one component in this composition.
    pub fn activity(&self, index: usize) -> ComponentActivity {
        if self.z[index] == 0.0 {
            ComponentActivity::Absent
        } else {
            ComponentActivity::Present
        }
    }

    /// Indices of the components carrying material.
    pub fn active_indices(&self) -> Vec<usize> {
        (0..self.z.len())
            .filter(|&i| self.activity(i) == ComponentActivity::Present)
            .collect()
    }

    /// Apply the same permutation that [`FluidSpecification::permuted`] applied.
    pub fn permuted(&self, order: &[usize]) -> Result<Self, FluidSpecError> {
        let n = self.z.len();
        let invalid = || FluidSpecError::InvalidPermutation {
            order: order.to_vec(),
            count: n,
        };
        if order.len() != n {
            return Err(invalid());
        }
        let mut seen = vec![false; n];
        for &o in order {
            if o >= n || seen[o] {
                return Err(invalid());
            }
            seen[o] = true;
        }
        Ok(Self {
            z: order.iter().map(|&o| self.z[o]).collect(),
        })
    }
}
