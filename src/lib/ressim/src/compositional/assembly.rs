//! Global residual and Jacobian assembly (compositional plan, C9).
//!
//! Walks the cells for [`super::accumulation`]'s terms and the faces for [`super::flux`]'s, and
//! places both into one system indexed by [`super::layout::CompositionalLayout`].
//!
//! ```text
//! R_i(cell) = n_i(new) - n_i(previous) + dt * sum_faces(outward flux_i) - dt * source_i
//! ```
//!
//! Sign convention: a face's flux is positive from its first cell to its second, so it is
//! **added** to the first cell's residual and **subtracted** from the second's. One number,
//! two signs — which is why an internal face cannot create or destroy material regardless of
//! whether its magnitude is right.
//!
//! The Jacobian is dense here because the grids C9 validates on are tiny and a dense comparison
//! against a numerical Jacobian is the point. The sparse structure the solver needs is C10's, and
//! it reads its block sizes and tail offsets from the same layout.

use crate::fluid::specification::FluidSpecification;

use super::accumulation::{AccumulationError, CellInventory, EquationScaling, cell_accumulation};
use super::flux::{FluxError, Gravity, face_flux};
use super::layout::CompositionalLayout;
use super::relperm::RelativePermeabilityModel;
use super::state::{CompositionalCellState, CompositionalState, RockView};

/// One connection between two cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    pub cell_i: usize,
    pub cell_j: usize,
    /// `DARCY_METRIC_FACTOR * geometric_transmissibility` [m³/day/bar per 1/cP].
    pub geom_t: f64,
    /// Gravity for this face. [`Gravity::OFF`] is V1's default.
    pub gravity: Gravity,
}

/// Why assembly failed.
#[derive(Clone, Debug, PartialEq)]
pub enum AssemblyError {
    Accumulation {
        cell: usize,
        source: AccumulationError,
    },
    Flux {
        face: usize,
        source: FluxError,
    },
    /// A face names a cell that is not in the grid, or connects a cell to itself.
    InvalidFace {
        face: usize,
        reason: &'static str,
    },
    /// The source or previous-inventory table does not match the grid.
    ShapeMismatch {
        what: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl core::fmt::Display for AssemblyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Accumulation { cell, source } => write!(f, "cell {cell}: {source}"),
            Self::Flux { face, source } => write!(f, "face {face}: {source}"),
            Self::InvalidFace { face, reason } => write!(f, "face {face}: {reason}"),
            Self::ShapeMismatch {
                what,
                expected,
                actual,
            } => {
                write!(f, "{what}: expected {expected}, got {actual}")
            }
        }
    }
}

/// The assembled system.
#[derive(Clone, Debug, PartialEq)]
pub struct AssemblyResult {
    /// Unscaled residual, one entry per cell equation [mol].
    pub residual: Vec<f64>,
    /// Unscaled Jacobian, `cells*N` square over the cell block. Well and perforation rows are
    /// C11's and are not present.
    pub jacobian: Vec<Vec<f64>>,
    /// Per-cell inventories at the current primaries.
    pub inventories: Vec<CellInventory>,
    /// Per-cell scaling, built from the previous inventory.
    pub scaling: Vec<EquationScaling>,
    /// Net component moles that crossed every face, summed with sign. Zero for a closed grid, by
    /// construction — reported so a caller can check rather than trust.
    pub net_internal_face_moles: Vec<f64>,
}

impl AssemblyResult {
    /// Largest scaled residual across every cell and component.
    pub fn scaled_residual_norm(&self, layout: &CompositionalLayout) -> f64 {
        let n = layout.component_count();
        let mut worst = 0.0f64;
        for cell in 0..layout.cell_count() {
            let rows = &self.residual[cell * n..(cell + 1) * n];
            worst = worst.max(self.scaling[cell].scaled_residual_norm(rows));
        }
        worst
    }
}

/// Assemble the residual and Jacobian for a whole grid.
///
/// `previous_moles` is the previous accepted state's inventory, one row per cell, evaluated once
/// by the caller. `sources` is in mol/day, positive for injection.
#[allow(clippy::too_many_arguments)]
pub fn assemble(
    spec: &FluidSpecification,
    layout: &CompositionalLayout,
    rock: &RockView<'_>,
    relperm: &RelativePermeabilityModel,
    state: &CompositionalState,
    faces: &[Face],
    previous_moles: &[Vec<f64>],
    sources: &[Vec<f64>],
    dt_days: f64,
) -> Result<AssemblyResult, AssemblyError> {
    let n = layout.component_count();
    let cells = layout.cell_count();

    if previous_moles.len() != cells {
        return Err(AssemblyError::ShapeMismatch {
            what: "previous inventory rows",
            expected: cells,
            actual: previous_moles.len(),
        });
    }
    if sources.len() != cells {
        return Err(AssemblyError::ShapeMismatch {
            what: "source rows",
            expected: cells,
            actual: sources.len(),
        });
    }
    for (index, face) in faces.iter().enumerate() {
        if face.cell_i >= cells || face.cell_j >= cells {
            return Err(AssemblyError::InvalidFace {
                face: index,
                reason: "names a cell outside the grid",
            });
        }
        if face.cell_i == face.cell_j {
            return Err(AssemblyError::InvalidFace {
                face: index,
                reason: "connects a cell to itself",
            });
        }
    }

    let size = cells * n;
    let mut residual = vec![0.0; size];
    let mut jacobian = vec![vec![0.0; size]; size];
    let mut inventories = Vec::with_capacity(cells);
    let mut scaling = Vec::with_capacity(cells);

    // Accumulation and sources, cell by cell.
    for cell in 0..cells {
        let acc = cell_accumulation(
            spec,
            rock,
            cell,
            state.cell(cell),
            &previous_moles[cell],
            &sources[cell],
            dt_days,
        )
        .map_err(|source| AssemblyError::Accumulation { cell, source })?;

        for i in 0..n {
            residual[cell * n + i] = acc.residual[i];
            for v in 0..n {
                jacobian[cell * n + i][cell * n + v] = acc.jacobian[i][v];
            }
        }
        scaling.push(EquationScaling::from_previous_inventory(
            &previous_moles[cell],
            state.cell(cell).pressure_bar,
        ));
        inventories.push(acc.inventory);
    }

    // Faces: one flux, inserted with two signs. The per-cell contributions are tallied separately
    // so the closure check below sums something that was actually assembled, rather than an
    // expression that is zero by inspection.
    let mut face_contributions = vec![0.0; size];
    for (index, face) in faces.iter().enumerate() {
        let flux = face_flux(
            spec,
            relperm,
            face.geom_t,
            face.gravity,
            (face.cell_i, state.cell(face.cell_i)),
            (face.cell_j, state.cell(face.cell_j)),
        )
        .map_err(|source| AssemblyError::Flux {
            face: index,
            source,
        })?;

        for i in 0..n {
            let contribution = dt_days * flux.component_moles_per_day[i];
            residual[face.cell_i * n + i] += contribution;
            residual[face.cell_j * n + i] -= contribution;
            face_contributions[face.cell_i * n + i] += contribution;
            face_contributions[face.cell_j * n + i] -= contribution;

            // Columns 0..N are cell_i's primaries, N..2N are cell_j's.
            for v in 0..n {
                let d_i = dt_days * flux.jacobian[i][v];
                let d_j = dt_days * flux.jacobian[i][n + v];
                jacobian[face.cell_i * n + i][face.cell_i * n + v] += d_i;
                jacobian[face.cell_i * n + i][face.cell_j * n + v] += d_j;
                jacobian[face.cell_j * n + i][face.cell_i * n + v] -= d_i;
                jacobian[face.cell_j * n + i][face.cell_j * n + v] -= d_j;
            }
        }
    }

    // Summed over the whole grid, the face contributions must cancel: every internal face put the
    // same number into two cells with opposite signs. Structurally guaranteed, and reported so a
    // caller can check the structure rather than trust it.
    let mut net_internal_face_moles = vec![0.0; n];
    for cell in 0..cells {
        for i in 0..n {
            net_internal_face_moles[i] += face_contributions[cell * n + i];
        }
    }

    Ok(AssemblyResult {
        residual,
        jacobian,
        inventories,
        scaling,
        net_internal_face_moles,
    })
}

/// A numerical Jacobian of [`assemble`], for comparison.
///
/// Deliberately built by perturbing the **state** and re-running the whole assembler, not by
/// perturbing anything inside it. That is what makes it independent: a shared helper between the
/// analytic and numerical paths would let one error cancel the other.
///
/// The previous inventory is held fixed across perturbations, as it must be — it is a constant of
/// the timestep, and letting it move would measure a different derivative.
#[allow(clippy::too_many_arguments)]
pub fn numerical_jacobian(
    spec: &FluidSpecification,
    layout: &CompositionalLayout,
    rock: &RockView<'_>,
    relperm: &RelativePermeabilityModel,
    state: &CompositionalState,
    faces: &[Face],
    previous_moles: &[Vec<f64>],
    sources: &[Vec<f64>],
    dt_days: f64,
    pressure_step_bar: f64,
    composition_step: f64,
) -> Result<Vec<Vec<f64>>, AssemblyError> {
    let n = layout.component_count();
    let cells = layout.cell_count();
    let size = cells * n;
    let mut out = vec![vec![0.0; size]; size];

    for column in 0..size {
        let (cell, primary) = layout
            .split_cell_unknown(column)
            .expect("every column in the cell block splits");

        let perturb = |sign: f64| -> Result<Vec<f64>, AssemblyError> {
            let mut cells_copy: Vec<CompositionalCellState> = state.cells().to_vec();
            match primary {
                super::layout::CellPrimary::Pressure => {
                    cells_copy[cell].pressure_bar += sign * pressure_step_bar;
                }
                super::layout::CellPrimary::Composition(k) => {
                    cells_copy[cell].independent_z_mut()[k] += sign * composition_step;
                }
            }
            let perturbed = CompositionalState::new(layout, cells_copy)
                .expect("a perturbed state must stay admissible");
            let result = assemble(
                spec,
                layout,
                rock,
                relperm,
                &perturbed,
                faces,
                previous_moles,
                sources,
                dt_days,
            )?;
            Ok(result.residual)
        };

        let up = perturb(1.0)?;
        let down = perturb(-1.0)?;
        let step = match primary {
            super::layout::CellPrimary::Pressure => pressure_step_bar,
            super::layout::CellPrimary::Composition(_) => composition_step,
        };
        for row in 0..size {
            out[row][column] = (up[row] - down[row]) / (2.0 * step);
        }
    }

    Ok(out)
}
