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
//! # Wells are implicit, not a source term
//!
//! A well's contribution is **re-evaluated at every Newton iterate** and its derivative with
//! respect to the connected cell's primaries goes into the Jacobian. It is not a constant
//! computed before the step.
//!
//! That distinction is not academic. A BHP well is a strong negative feedback — as a cell
//! pressurizes, injection into it must fall — and an explicitly evaluated source has no feedback
//! at all. Holding the source fixed over a step lets the cell overshoot its injector's BHP
//! entirely: the C12 transport comparison saw a cell reach 238 bar against a 150 bar injector and
//! then oscillate by ±60 bar between report times. Nothing in a unit test caught it, because every
//! unit test imposes a source rather than a well.
//!
//! `sources` remains for genuinely imposed material — a test fixture, or a boundary condition that
//! is not a well — and is constant by definition.
//!
//! The Jacobian is stored block-sparse ([`BlockSparseMatrix`]): one `N x N` block per cell and per
//! face neighbour, which is every entry assembly can write. C9 validated it dense against a
//! numerical Jacobian on tiny grids; [`BlockSparseMatrix::to_dense`] keeps that comparison exact,
//! and nothing at SPE scale ever allocates the dense square.

use crate::fluid::specification::FluidSpecification;

use super::accumulation::{
    AccumulationError, CellInventory, EquationScaling, cell_accumulation_with_water,
};
use super::block_matrix::BlockSparseMatrix;
use super::flux::{FluxError, Gravity, face_flux};
use super::layout::CompositionalLayout;
use super::relperm::RelativePermeabilityModel;
use super::state::{CompositionalCellState, CompositionalState, RockView};
use super::wells::{CompositionalWell, WellError, evaluate_well_control, initial_bhp_guesses};

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
    /// A well could not be evaluated at the current iterate.
    Well {
        well: String,
        source: WellError,
    },
    /// A rate-control row has no usable derivative with respect to its BHP unknown, so its BHP
    /// cannot be eliminated from the Newton system.
    WellConstraintSingular {
        well: String,
        derivative: f64,
    },
    /// A face names a cell that is not in the grid, or connects a cell to itself.
    InvalidFace {
        face: usize,
        reason: &'static str,
    },
    /// The layout has water and the relative permeability does not, or the reverse.
    ///
    /// The water phase is carried by [`RelativePermeabilityModel::StoneTwoWater`], and a layout
    /// that disagrees with it would put a water row where no physics fills it.
    WaterMismatch {
        layout_has_water: bool,
        relperm_has_water: bool,
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
            Self::Well { well, source } => write!(f, "well {well}: {source}"),
            Self::WellConstraintSingular { well, derivative } => write!(
                f,
                "well {well}: its rate-control row has d(rate)/d(bhp) = {derivative:e}, so the \
                 BHP unknown cannot be eliminated"
            ),
            Self::InvalidFace { face, reason } => write!(f, "face {face}: {reason}"),
            Self::WaterMismatch {
                layout_has_water,
                relperm_has_water,
            } => write!(
                f,
                "layout water = {layout_has_water}, relative permeability water = \
                 {relperm_has_water}; the two must agree"
            ),
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
    /// Unscaled Jacobian, `cells*N` square over the cell block, stored by `N x N` blocks over the
    /// face graph. A rate-controlled well's BHP unknown is eliminated by
    /// [`Self::reduce_well_unknowns`] before the linear solve, so there are no well rows here.
    pub jacobian: BlockSparseMatrix,
    /// Each cell's accumulation block `dn_i/du_v`, row-major `N x N`, unscaled. Already inside
    /// `jacobian`; kept separately because the CPR pressure weights are defined by it alone.
    pub accumulation_blocks: Vec<Vec<f64>>,
    /// Per-cell inventories at the current primaries.
    pub inventories: Vec<CellInventory>,
    /// Per-cell scaling, built from the previous inventory.
    pub scaling: Vec<EquationScaling>,
    /// Net component moles that crossed every face, summed with sign. Zero for a closed grid, by
    /// construction — reported so a caller can check rather than trust.
    pub net_internal_face_moles: Vec<f64>,
    /// Each well's component rate at this iterate [mol/day], in the order the wells were given.
    ///
    /// Reported because a well's rate is a result of the solve rather than an input to it, and a
    /// caller accumulating production needs the rate that was actually accepted.
    pub well_rates: Vec<Vec<f64>>,
    /// The BHP each well was evaluated at, in the order the wells were given.
    pub well_bhp: Vec<f64>,
    /// Whether each well was on its BHP limit at this iterate.
    pub well_on_bhp_limit: Vec<bool>,
    /// One rate-control row per well whose rate equation is active. The BHP unknown is eliminated
    /// locally by [`Self::reduce_well_unknowns`]; this is everything that elimination needs.
    pub well_eliminations: Vec<WellElimination>,
}

/// A rate-control row and its coupling to one well's BHP unknown.
///
/// `cell_bhp_column` is `d(cell residual)/d(bhp)` over the whole cell block, and `cell_jacobian`
/// is `d(rate residual)/d(cell primaries)` restricted to the connected cells. The BHP block is
/// scalar and diagonal — a well's own constraint never depends on another well's BHP — so the
/// elimination is a rank-one update per well rather than a general Schur solve.
#[derive(Clone, Debug, PartialEq)]
pub struct WellElimination {
    /// Index into the well slice.
    pub well: usize,
    /// Stable id for the failure report.
    pub id: String,
    /// `achieved - target` at this iterate.
    pub residual: f64,
    /// `|target|`, floored at one, used for the convergence test.
    pub scale: f64,
    /// `d residual / d bhp`.
    pub bhp_derivative: f64,
    /// `d residual / d u` for each connected cell.
    pub cell_jacobian: Vec<(usize, Vec<f64>)>,
    /// `d(cell residual)/d(bhp)` over the flat cell block.
    pub cell_bhp_column: Vec<f64>,
}

impl AssemblyResult {
    /// Largest scaled residual across every cell, component and active rate constraint.
    pub fn scaled_residual_norm(&self, layout: &CompositionalLayout) -> f64 {
        let n = layout.cell_block_size();
        let mut worst = 0.0f64;
        for cell in 0..layout.cell_count() {
            let rows = &self.residual[cell * n..(cell + 1) * n];
            worst = worst.max(self.scaling[cell].scaled_residual_norm(rows));
        }
        for elimination in &self.well_eliminations {
            worst = worst.max(elimination.residual.abs() / elimination.scale);
        }
        worst
    }

    /// Eliminate the active rate-control BHPs, returning the reduced cell system.
    ///
    /// ```text
    /// [J_cc  J_cw] [dx_c]   [-R_c]
    /// [J_wc  J_ww] [dx_w] = [-R_w]
    /// ```
    ///
    /// With one scalar BHP per rate-controlled well and `J_ww` diagonal, the reduced system is
    /// `(J_cc - J_cw J_ww^-1 J_wc) dx_c = -R_c + J_cw J_ww^-1 R_w`. The update is applied to the
    /// diagonal blocks only, because a completion's BHP column and its rate row are both
    /// restricted to its connected cells.
    pub fn reduce_well_unknowns(&self) -> Result<(BlockSparseMatrix, Vec<f64>), AssemblyError> {
        let mut jacobian = self.jacobian.clone();
        let mut residual = self.residual.clone();
        let n = jacobian.block_size();
        for elimination in &self.well_eliminations {
            let a = elimination.bhp_derivative;
            if !a.is_finite() || a == 0.0 {
                return Err(AssemblyError::WellConstraintSingular {
                    well: elimination.id.clone(),
                    derivative: a,
                });
            }
            let residual_over_a = elimination.residual / a;
            for (cell, row) in &elimination.cell_jacobian {
                let diagonal = jacobian.diagonal_position(*cell);
                for i in 0..n {
                    let column = elimination.cell_bhp_column[cell * n + i];
                    if column == 0.0 {
                        continue;
                    }
                    for v in 0..n {
                        jacobian.block_mut(diagonal)[i * n + v] -= column * row[v] / a;
                    }
                    residual[cell * n + i] -= column * residual_over_a;
                }
            }
        }
        Ok((jacobian, residual))
    }

    /// Recover `dx_w` for each active rate-control BHP after the reduced system has been solved.
    pub fn recover_well_updates(&self, dx_cells: &[f64]) -> Result<Vec<f64>, AssemblyError> {
        let n = self.jacobian.block_size();
        let mut updates = Vec::with_capacity(self.well_eliminations.len());
        for elimination in &self.well_eliminations {
            let a = elimination.bhp_derivative;
            if !a.is_finite() || a == 0.0 {
                return Err(AssemblyError::WellConstraintSingular {
                    well: elimination.id.clone(),
                    derivative: a,
                });
            }
            let mut j_wc_dx = 0.0;
            for (cell, row) in &elimination.cell_jacobian {
                for v in 0..n {
                    j_wc_dx += row[v] * dx_cells[cell * n + v];
                }
            }
            updates.push((-elimination.residual - j_wc_dx) / a);
        }
        Ok(updates)
    }
}

/// Assemble with a fresh BHP guess for every well.
///
/// The Newton loop calls [`assemble_with_well_bhp`] instead, because its BHP is an iterate it
/// owns. This wrapper exists for callers that only need the cell rows at a self-consistent guess
/// — an imposed-source test, or a diagnostic that asks what one assembly would look like.
#[allow(clippy::too_many_arguments)]
pub fn assemble(
    spec: &FluidSpecification,
    layout: &CompositionalLayout,
    rock: &RockView<'_>,
    relperm: &RelativePermeabilityModel,
    state: &CompositionalState,
    faces: &[Face],
    wells: &[CompositionalWell],
    previous_moles: &[Vec<f64>],
    sources: &[Vec<f64>],
    dt_days: f64,
) -> Result<AssemblyResult, AssemblyError> {
    let well_bhp = initial_bhp_guesses(wells, state.cells());
    assemble_with_well_bhp(
        spec,
        layout,
        rock,
        relperm,
        state,
        faces,
        wells,
        previous_moles,
        sources,
        dt_days,
        &well_bhp,
    )
}

/// Assemble the residual and Jacobian for a whole grid at a given set of well BHPs.
///
/// `previous_moles` is the previous accepted state's inventory, one row per cell, evaluated once
/// by the caller. `sources` is in mol/day, positive for injection. `well_bhp` has one entry per
/// well; an active rate control contributes a constraint row that
/// [`AssemblyResult::reduce_well_unknowns`] eliminates.
#[allow(clippy::too_many_arguments)]
pub fn assemble_with_well_bhp(
    spec: &FluidSpecification,
    layout: &CompositionalLayout,
    rock: &RockView<'_>,
    relperm: &RelativePermeabilityModel,
    state: &CompositionalState,
    faces: &[Face],
    wells: &[CompositionalWell],
    previous_moles: &[Vec<f64>],
    sources: &[Vec<f64>],
    dt_days: f64,
    well_bhp: &[f64],
) -> Result<AssemblyResult, AssemblyError> {
    // Rows and columns per cell: `N`, or `N+1` with water.
    let n = layout.cell_block_size();
    let cells = layout.cell_count();
    let water = relperm.water().map(|w| &w.properties);
    if layout.has_water() != water.is_some() {
        return Err(AssemblyError::WaterMismatch {
            layout_has_water: layout.has_water(),
            relperm_has_water: water.is_some(),
        });
    }

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
    if well_bhp.len() != wells.len() {
        return Err(AssemblyError::ShapeMismatch {
            what: "well BHP values",
            expected: wells.len(),
            actual: well_bhp.len(),
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
    let mut jacobian =
        BlockSparseMatrix::from_connections(cells, n, faces.iter().map(|f| (f.cell_i, f.cell_j)))
            .ok_or(AssemblyError::InvalidFace {
            face: 0,
            reason: "the face graph does not form a Jacobian pattern",
        })?;
    let mut accumulation_blocks = Vec::with_capacity(cells);
    let mut inventories = Vec::with_capacity(cells);
    let mut scaling = Vec::with_capacity(cells);

    // Accumulation and sources, cell by cell.
    for cell in 0..cells {
        let acc = cell_accumulation_with_water(
            spec,
            rock,
            water,
            cell,
            state.cell(cell),
            &previous_moles[cell],
            &sources[cell],
            dt_days,
        )
        .map_err(|source| AssemblyError::Accumulation { cell, source })?;

        let mut block = vec![0.0; n * n];
        for i in 0..n {
            residual[cell * n + i] = acc.residual[i];
            block[i * n..(i + 1) * n].copy_from_slice(&acc.jacobian[i]);
        }
        let diagonal = jacobian.diagonal_position(cell);
        jacobian.block_mut(diagonal).copy_from_slice(&block);
        accumulation_blocks.push(block);
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

        let ii = jacobian.diagonal_position(face.cell_i);
        let jj = jacobian.diagonal_position(face.cell_j);
        let (Some(ij), Some(ji)) = (
            jacobian.position(face.cell_i, face.cell_j),
            jacobian.position(face.cell_j, face.cell_i),
        ) else {
            return Err(AssemblyError::InvalidFace {
                face: index,
                reason: "is not in the Jacobian pattern",
            });
        };

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
                jacobian.block_mut(ii)[i * n + v] += d_i;
                jacobian.block_mut(ij)[i * n + v] += d_j;
                jacobian.block_mut(ji)[i * n + v] -= d_i;
                jacobian.block_mut(jj)[i * n + v] -= d_j;
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

    // Wells, evaluated at **this** iterate. Their derivative with respect to the connected cell's
    // primaries is what makes the BHP feedback part of the Newton step rather than a constant.
    // A rate-controlled well also contributes one scalar equation with its BHP as the unknown;
    // that row is collected here and eliminated by `reduce_well_unknowns`.
    let mut well_rates = Vec::with_capacity(wells.len());
    let mut well_bhp_values = Vec::with_capacity(wells.len());
    let mut well_on_bhp_limit = Vec::with_capacity(wells.len());
    let mut well_eliminations = Vec::new();
    for (well_index, well) in wells.iter().enumerate() {
        let evaluation =
            evaluate_well_control(spec, relperm, well, state.cells(), well_bhp[well_index])
                .map_err(|source| AssemblyError::Well {
                    well: well.id.clone(),
                    source,
                })?;
        let result = evaluation.result;
        well_rates.push(result.total_component_moles_per_day(n));
        well_bhp_values.push(result.bhp_bar);
        well_on_bhp_limit.push(result.on_bhp_limit);

        for connection in &result.completions {
            let cell = connection.cell;
            let diagonal = jacobian.diagonal_position(cell);
            for i in 0..n {
                residual[cell * n + i] -= dt_days * connection.component_moles_per_day[i];
                for v in 0..n {
                    jacobian.block_mut(diagonal)[i * n + v] -=
                        dt_days * connection.cell_jacobian[i][v];
                }
            }
        }

        if let Some(constraint) = evaluation.constraint {
            let mut cell_bhp_column = vec![0.0; size];
            for connection in &result.completions {
                for i in 0..n {
                    cell_bhp_column[connection.cell * n + i] -=
                        dt_days * connection.bhp_derivative[i];
                }
            }
            well_eliminations.push(WellElimination {
                well: well_index,
                id: well.id.clone(),
                residual: constraint.residual,
                scale: constraint.scale,
                bhp_derivative: constraint.bhp_derivative,
                cell_jacobian: constraint.cell_jacobian,
                cell_bhp_column,
            });
        }
    }

    Ok(AssemblyResult {
        residual,
        jacobian,
        accumulation_blocks,
        inventories,
        scaling,
        net_internal_face_moles,
        well_rates,
        well_bhp: well_bhp_values,
        well_on_bhp_limit,
        well_eliminations,
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
    wells: &[CompositionalWell],
    previous_moles: &[Vec<f64>],
    sources: &[Vec<f64>],
    dt_days: f64,
    pressure_step_bar: f64,
    composition_step: f64,
) -> Result<Vec<Vec<f64>>, AssemblyError> {
    let n = layout.cell_block_size();
    let cells = layout.cell_count();
    let size = cells * n;
    let mut out = vec![vec![0.0; size]; size];
    // Hold every well's BHP fixed across perturbations: the analytic matrix compared here is the
    // cell block before the rate-control rows eliminate their BHP unknowns. Letting the guess
    // follow the perturbed state would measure a different derivative.
    let well_bhp = initial_bhp_guesses(wells, state.cells());

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
                super::layout::CellPrimary::WaterSaturation => {
                    cells_copy[cell].water_saturation += sign * composition_step;
                }
            }
            let perturbed = CompositionalState::new(layout, cells_copy)
                .expect("a perturbed state must stay admissible");
            let result = assemble_with_well_bhp(
                spec,
                layout,
                rock,
                relperm,
                &perturbed,
                faces,
                wells,
                previous_moles,
                sources,
                dt_days,
                &well_bhp,
            )?;
            Ok(result.residual)
        };

        let up = perturb(1.0)?;
        let down = perturb(-1.0)?;
        let step = match primary {
            super::layout::CellPrimary::Pressure => pressure_step_bar,
            super::layout::CellPrimary::Composition(_)
            | super::layout::CellPrimary::WaterSaturation => composition_step,
        };
        for row in 0..size {
            out[row][column] = (up[row] - down[row]) / (2.0 * step);
        }
    }

    Ok(out)
}
