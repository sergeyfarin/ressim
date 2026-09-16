//! Compositional state, the flash cache and checkpoints (compositional plan, C7).
//!
//! # Accepted versus trial
//!
//! [`CompositionalState`] is the **accepted** state: the last one that passed every convergence
//! and admissibility check. [`TrialState`] is a candidate produced by a Newton step. They are
//! separate types, and the only way from one to the other is [`TrialState::commit`].
//!
//! That is deliberate. The plan requires that a rejected step mutate nothing — not time, not
//! inventory, not well cumulatives, not the flash cache — and the reliable way to guarantee that
//! is to make the accepted state unreachable from a trial without an explicit commit. A single
//! mutable state that a solver edits in place and rolls back on failure is the design where
//! "rolled back everything" is a claim rather than a property.
//!
//! # Composition is stored with one fewer entry than it has components
//!
//! A cell stores `p` and `z_0 .. z_(N-2)`. The dependent `z_(N-1)` is computed, never stored, so
//! `sum z = 1` cannot be broken by an update — there is no second copy to drift. A Newton step
//! that moves the independent coordinates moves the dependent one exactly as the derivative
//! contract in C4 says it does.
//!
//! # The flash cache is a hint
//!
//! [`FlashCache`] keys on the **exact bit pattern** of the primaries it was computed from. A hit
//! means "these are the same numbers", not "these are close enough", because a cached equilibrium
//! for nearly-the-same state is a wrong equilibrium, and the plan is explicit that caches are
//! performance hints that cannot determine physical truth. Everything the cache holds is
//! recomputable from the primaries; nothing depends on it being present.

use crate::fluid::flash::FlashState;
use serde::{Deserialize, Serialize};

use super::layout::CompositionalLayout;

/// Monotonic version stamp for an accepted state.
///
/// Bumped on every commit. A cache or checkpoint carrying a different version describes a
/// different state, whatever its contents look like.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StateVersion(pub u64);

/// Why a state operation failed.
#[derive(Clone, Debug, PartialEq)]
pub enum StateError {
    /// A cell's independent composition vector is the wrong length.
    CompositionLength {
        cell: usize,
        expected: usize,
        actual: usize,
    },
    /// A primary is non-finite, or a composition coordinate is negative.
    NonPhysicalPrimary {
        cell: usize,
        what: &'static str,
        value: f64,
    },
    /// The independent coordinates sum above one, so the dependent component would be negative.
    DependentCompositionNegative { cell: usize, dependent: f64 },
    /// Pressure is not strictly positive.
    NonPositivePressure { cell: usize, pressure_bar: f64 },
    /// A cell count or component count does not match the layout.
    LayoutMismatch {
        what: &'static str,
        expected: usize,
        actual: usize,
    },
    /// A checkpoint's schema version is not the one this build writes.
    IncompatibleCheckpoint { found: u32, expected: u32 },
}

impl core::fmt::Display for StateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::CompositionLength {
                cell,
                expected,
                actual,
            } => write!(
                f,
                "cell {cell} has {actual} independent composition entries, expected {expected}"
            ),
            Self::NonPhysicalPrimary { cell, what, value } => {
                write!(f, "cell {cell}: {what} = {value} is not physical")
            }
            Self::DependentCompositionNegative { cell, dependent } => write!(
                f,
                "cell {cell}: the independent compositions sum above one, leaving a dependent \
                 component of {dependent}"
            ),
            Self::NonPositivePressure { cell, pressure_bar } => {
                write!(
                    f,
                    "cell {cell}: pressure {pressure_bar} bar is not positive"
                )
            }
            Self::LayoutMismatch {
                what,
                expected,
                actual,
            } => {
                write!(f, "{what}: expected {expected}, got {actual}")
            }
            Self::IncompatibleCheckpoint { found, expected } => write!(
                f,
                "checkpoint schema {found} cannot be read by a build that writes {expected}"
            ),
        }
    }
}

/// One cell's primary unknowns.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompositionalCellState {
    /// Cell pressure [bar]. Reservoir units here; the EOS boundary converts.
    pub pressure_bar: f64,
    /// `z_0 .. z_(N-2)`. The dependent component is **not** stored.
    independent_z: Vec<f64>,
}

impl CompositionalCellState {
    /// Build and validate a cell state from its independent coordinates.
    pub fn new(pressure_bar: f64, independent_z: Vec<f64>) -> Result<Self, StateError> {
        let state = Self {
            pressure_bar,
            independent_z,
        };
        state.validate(0)?;
        Ok(state)
    }

    /// Independent coordinates, `N-1` entries.
    pub fn independent_z(&self) -> &[f64] {
        &self.independent_z
    }

    /// Mutable independent coordinates, for a Newton update.
    ///
    /// Available only through a [`TrialState`]'s `cell_mut`, so the accepted state stays
    /// immutable. Length cannot change through this accessor, which is what keeps the layout and
    /// the state in step.
    pub fn independent_z_mut(&mut self) -> &mut [f64] {
        &mut self.independent_z
    }

    /// Dependent overall mole fraction, `1 - sum(z_0..z_(N-2))`.
    pub fn dependent_z(&self) -> f64 {
        1.0 - self.independent_z.iter().sum::<f64>()
    }

    /// The full overall composition, `N` entries.
    ///
    /// Materialized on demand rather than stored, so it cannot drift out of step with the
    /// independent coordinates.
    pub fn overall_composition(&self) -> Vec<f64> {
        let mut z = self.independent_z.clone();
        z.push(self.dependent_z());
        z
    }

    fn validate(&self, cell: usize) -> Result<(), StateError> {
        if !self.pressure_bar.is_finite() {
            return Err(StateError::NonPhysicalPrimary {
                cell,
                what: "pressure_bar",
                value: self.pressure_bar,
            });
        }
        if self.pressure_bar <= 0.0 {
            return Err(StateError::NonPositivePressure {
                cell,
                pressure_bar: self.pressure_bar,
            });
        }
        for &value in &self.independent_z {
            if !value.is_finite() || value < 0.0 {
                return Err(StateError::NonPhysicalPrimary {
                    cell,
                    what: "independent z",
                    value,
                });
            }
        }
        let dependent = self.dependent_z();
        if dependent < 0.0 {
            return Err(StateError::DependentCompositionNegative { cell, dependent });
        }
        Ok(())
    }
}

/// The accepted state of every cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompositionalState {
    component_count: usize,
    cells: Vec<CompositionalCellState>,
    version: StateVersion,
}

impl CompositionalState {
    /// Validate and construct an accepted state against a layout.
    pub fn new(
        layout: &CompositionalLayout,
        cells: Vec<CompositionalCellState>,
    ) -> Result<Self, StateError> {
        if cells.len() != layout.cell_count() {
            return Err(StateError::LayoutMismatch {
                what: "cell count",
                expected: layout.cell_count(),
                actual: cells.len(),
            });
        }
        let expected = layout.independent_composition_count();
        for (index, cell) in cells.iter().enumerate() {
            if cell.independent_z.len() != expected {
                return Err(StateError::CompositionLength {
                    cell: index,
                    expected,
                    actual: cell.independent_z.len(),
                });
            }
            cell.validate(index)?;
        }
        Ok(Self {
            component_count: layout.component_count(),
            cells,
            version: StateVersion(0),
        })
    }

    pub fn component_count(&self) -> usize {
        self.component_count
    }

    pub fn cells(&self) -> &[CompositionalCellState] {
        &self.cells
    }

    pub fn cell(&self, index: usize) -> &CompositionalCellState {
        &self.cells[index]
    }

    pub fn version(&self) -> StateVersion {
        self.version
    }

    /// Begin a Newton trial from this accepted state.
    ///
    /// The trial owns a copy. Editing it cannot reach the accepted state, which is what makes
    /// "a rejected step changed nothing" a property rather than a claim.
    pub fn begin_trial(&self) -> TrialState {
        TrialState {
            base_version: self.version,
            component_count: self.component_count,
            cells: self.cells.clone(),
        }
    }

    /// A versioned checkpoint of this state.
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            schema: CHECKPOINT_SCHEMA,
            component_count: self.component_count,
            version: self.version,
            cells: self.cells.clone(),
        }
    }

    /// Restore from a checkpoint, rejecting an incompatible schema.
    pub fn from_checkpoint(
        layout: &CompositionalLayout,
        checkpoint: Checkpoint,
    ) -> Result<Self, StateError> {
        if checkpoint.schema != CHECKPOINT_SCHEMA {
            return Err(StateError::IncompatibleCheckpoint {
                found: checkpoint.schema,
                expected: CHECKPOINT_SCHEMA,
            });
        }
        if checkpoint.component_count != layout.component_count() {
            return Err(StateError::LayoutMismatch {
                what: "component count",
                expected: layout.component_count(),
                actual: checkpoint.component_count,
            });
        }
        let mut restored = Self::new(layout, checkpoint.cells)?;
        restored.version = checkpoint.version;
        Ok(restored)
    }
}

/// A candidate state produced by a Newton step.
///
/// Mutable, validated only at [`Self::commit`], and unable to become the accepted state any other
/// way.
#[derive(Clone, Debug, PartialEq)]
pub struct TrialState {
    base_version: StateVersion,
    component_count: usize,
    cells: Vec<CompositionalCellState>,
}

impl TrialState {
    /// The accepted version this trial started from.
    pub fn base_version(&self) -> StateVersion {
        self.base_version
    }

    pub fn cells(&self) -> &[CompositionalCellState] {
        &self.cells
    }

    /// Mutable access to a cell, for the update step.
    pub fn cell_mut(&mut self, index: usize) -> &mut CompositionalCellState {
        &mut self.cells[index]
    }

    /// Validate and promote to the accepted state.
    ///
    /// Validation happens here rather than at each edit, so an update that passes through a
    /// transiently inadmissible intermediate is not rejected for it — only the state that is
    /// actually about to be accepted has to be admissible. The version advances past the base's,
    /// so anything keyed on the old version is invalidated by construction.
    pub fn commit(self) -> Result<CompositionalState, StateError> {
        for (index, cell) in self.cells.iter().enumerate() {
            cell.validate(index)?;
        }
        Ok(CompositionalState {
            component_count: self.component_count,
            cells: self.cells,
            version: StateVersion(self.base_version.0 + 1),
        })
    }
}

/// Checkpoint schema version.
///
/// Bumped whenever the serialized shape changes. A checkpoint from another schema is rejected, not
/// migrated: a restart that silently reinterprets old bytes is worse than one that refuses.
pub const CHECKPOINT_SCHEMA: u32 = 1;

/// A serializable, versioned snapshot of an accepted state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub schema: u32,
    pub component_count: usize,
    pub version: StateVersion,
    cells: Vec<CompositionalCellState>,
}

/// The exact primaries a cached flash was computed from.
///
/// Bit patterns, not values. Two states that differ in the last bit are different states, and a
/// cache that treated them as the same would return an equilibrium for a state nobody is at. The
/// state version is part of the key too, so a committed step invalidates every entry without
/// anything having to walk the cache.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FlashCacheKey {
    version: StateVersion,
    pressure_bits: u64,
    z_bits: Vec<u64>,
}

impl FlashCacheKey {
    pub fn new(version: StateVersion, cell: &CompositionalCellState) -> Self {
        Self {
            version,
            pressure_bits: cell.pressure_bar.to_bits(),
            z_bits: cell.independent_z().iter().map(|v| v.to_bits()).collect(),
        }
    }
}

/// Per-cell cached flash results.
///
/// Purely a performance hint. Every entry is recomputable from the primaries it is keyed on, and
/// dropping the whole cache changes no result — only the time taken. A test asserts exactly that.
#[derive(Clone, Debug, Default)]
pub struct FlashCache {
    entries: Vec<Option<(FlashCacheKey, FlashState)>>,
    hits: usize,
    misses: usize,
}

impl FlashCache {
    pub fn with_cells(cell_count: usize) -> Self {
        Self {
            entries: vec![None; cell_count],
            hits: 0,
            misses: 0,
        }
    }

    /// Look up a cell's flash, if one was stored for exactly these primaries.
    pub fn get(&mut self, cell: usize, key: &FlashCacheKey) -> Option<&FlashState> {
        let hit = matches!(&self.entries[cell], Some((stored, _)) if stored == key);
        if hit {
            self.hits += 1;
            self.entries[cell].as_ref().map(|(_, state)| state)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn insert(&mut self, cell: usize, key: FlashCacheKey, state: FlashState) {
        self.entries[cell] = Some((key, state));
    }

    /// Drop everything. Safe at any point, by construction.
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = None;
        }
    }

    pub fn hits(&self) -> usize {
        self.hits
    }

    pub fn misses(&self) -> usize {
        self.misses
    }
}

/// A read-only view of the rock properties the compositional accumulation needs.
///
/// Borrowed slices, not a `ReservoirSimulator`. The plan's C7 is explicit that EOS and
/// accumulation code must not be handed the whole simulator merely to reach a porosity, and this
/// is the boundary that enforces it: nothing reachable from here can start a timestep, touch a
/// well or read a saturation.
///
/// The pore-volume relation is the existing sourced one from `fim/properties.rs`:
/// `pv(p) = pv_ref * exp(c_rock * (p - p_ref))`, with pressures in bar and `c_rock` in 1/bar.
#[derive(Clone, Copy, Debug)]
pub struct RockView<'a> {
    /// Pore volume at the reference pressure [m³], one per cell.
    pub pore_volume_ref_m3: &'a [f64],
    /// The pressure the reference pore volumes are defined at [bar].
    pub reference_pressure_bar: f64,
    /// Rock compressibility [1/bar].
    pub compressibility_per_bar: f64,
}

impl RockView<'_> {
    /// Pore volume of a cell at pressure `p` [m³].
    pub fn pore_volume_m3(&self, cell: usize, pressure_bar: f64) -> f64 {
        self.pore_volume_ref_m3[cell]
            * ((pressure_bar - self.reference_pressure_bar) * self.compressibility_per_bar).exp()
    }

    pub fn cell_count(&self) -> usize {
        self.pore_volume_ref_m3.len()
    }
}
