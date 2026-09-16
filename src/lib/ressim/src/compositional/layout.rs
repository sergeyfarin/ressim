//! Component-aware matrix layout (compositional plan, C7).
//!
//! What lives in which matrix row and column, named rather than computed at the call site. This is
//! the compositional counterpart of [`crate::fim::layout`], and it exists for the same reason that
//! one does: the plan requires that a row's physical meaning never be inferred from `index % N`.
//!
//! # The block
//!
//! For `N` components each reservoir cell contributes `N` unknowns and `N` equations:
//!
//! ```text
//! columns:  [ p , z_0 , ... , z_(N-2) ]     N primaries; z_(N-1) = 1 - sum(z_0..z_(N-2))
//! rows:     [ R_0 , R_1 , ... , R_(N-1) ]   N component mole balances
//! ```
//!
//! The counts match because the dependent composition is *not* an unknown and pressure is *not* a
//! separate equation. Pressure enters through the EOS densities in every balance, so there is no
//! pressure closure to append; appending one would overdetermine the system.
//!
//! Columns and rows are numbered separately on purpose. They happen to share a block size here,
//! and that is a property of this model rather than a law — [`crate::fim::layout`] makes the same
//! distinction for the black-oil model, and for the same reason.
//!
//! # Pressure stays in local column 0
//!
//! Not cosmetic. `fim/linear`'s CPR restriction builds its coarse system from the cell pressure
//! column, and [`crate::fim::linear::FimLinearBlockLayout::is_cell_pressure_column`] identifies it
//! positionally. Keeping pressure first is what lets the compositional model reuse that machinery
//! without a second pressure-extraction path.
//!
//! # What this module does not do
//!
//! It does not touch the black-oil layout. `CELL_BLOCK_SIZE` there stays 3 and every existing
//! offset is unchanged; the two models have separate layouts because they have different
//! equations, not because the code was duplicated.

use crate::fim::linear::FimLinearBlockLayout;

/// Smallest supported component count.
pub const MIN_COMPONENTS: usize = 2;

/// Largest supported component count.
///
/// The pinned V1 fluids stop at 3. Four is admitted so C7 can carry a **layout-only** N=4 case,
/// which is what exposes an assumption that three components happen to satisfy — an offset written
/// as `+ 2` rather than `+ (n - 1)` passes every N=3 test and fails here.
pub const MAX_COMPONENTS: usize = 4;

/// A cell's primary unknowns, in matrix **column** order within its block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellPrimary {
    /// Cell pressure [bar]. Local column 0, always — see the module docs.
    Pressure,
    /// Overall mole fraction of an **independent** component, `0 <= k < N-1`.
    ///
    /// There is no variant for the dependent component, because it is not an unknown. That is the
    /// point: a type that could name `Composition(N-1)` would let a caller ask for the column of
    /// something that has no column.
    Composition(usize),
}

/// A cell's equations, in matrix **row** order within its block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellEquation {
    /// Mole balance for component `i`, `0 <= i < N`, including the dependent one.
    ///
    /// Every component gets a balance; only the compositions lose one degree of freedom.
    ComponentBalance(usize),
}

/// Where everything lives in the compositional Jacobian.
///
/// The tail layout — well BHP unknowns, then perforation rates — matches the black-oil one, so
/// `fim/linear`'s Schur elimination and row partitions apply unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompositionalLayout {
    component_count: usize,
    cell_count: usize,
    well_count: usize,
    perforation_count: usize,
}

/// Why a layout could not be constructed or an index could not be resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    /// Component count outside `MIN_COMPONENTS..=MAX_COMPONENTS`.
    UnsupportedComponentCount { count: usize },
    /// A composition index at or past `N-1`, which names the dependent component.
    ///
    /// Distinguished from an out-of-range index because the two mean different things: `N-1` is a
    /// real component that simply has no column, while `N` and beyond is not a component at all.
    DependentCompositionHasNoColumn {
        index: usize,
        component_count: usize,
    },
    /// An index beyond the component count.
    ComponentOutOfRange {
        index: usize,
        component_count: usize,
    },
    /// A cell index beyond the cell count.
    CellOutOfRange { index: usize, cell_count: usize },
}

impl core::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedComponentCount { count } => write!(
                f,
                "component count {count} is outside {MIN_COMPONENTS}..={MAX_COMPONENTS}"
            ),
            Self::DependentCompositionHasNoColumn {
                index,
                component_count,
            } => write!(
                f,
                "component {index} of {component_count} is the dependent composition and has no \
                 column; it is recovered as 1 - sum of the others"
            ),
            Self::ComponentOutOfRange {
                index,
                component_count,
            } => {
                write!(f, "component {index} is outside 0..{component_count}")
            }
            Self::CellOutOfRange { index, cell_count } => {
                write!(f, "cell {index} is outside 0..{cell_count}")
            }
        }
    }
}

impl CompositionalLayout {
    pub fn new(
        component_count: usize,
        cell_count: usize,
        well_count: usize,
        perforation_count: usize,
    ) -> Result<Self, LayoutError> {
        if !(MIN_COMPONENTS..=MAX_COMPONENTS).contains(&component_count) {
            return Err(LayoutError::UnsupportedComponentCount {
                count: component_count,
            });
        }
        Ok(Self {
            component_count,
            cell_count,
            well_count,
            perforation_count,
        })
    }

    pub fn component_count(&self) -> usize {
        self.component_count
    }

    pub fn cell_count(&self) -> usize {
        self.cell_count
    }

    pub fn well_count(&self) -> usize {
        self.well_count
    }

    pub fn perforation_count(&self) -> usize {
        self.perforation_count
    }

    /// Unknowns per cell, which equals equations per cell: `N`.
    pub fn cell_block_size(&self) -> usize {
        self.component_count
    }

    /// Number of **independent** composition unknowns per cell: `N-1`.
    pub fn independent_composition_count(&self) -> usize {
        self.component_count - 1
    }

    /// Total unknowns, cells plus the well and perforation tail.
    pub fn total_unknowns(&self) -> usize {
        self.cell_count * self.cell_block_size() + self.well_count + self.perforation_count
    }

    /// Local column of a primary within a cell block.
    pub fn local_column(&self, primary: CellPrimary) -> Result<usize, LayoutError> {
        match primary {
            CellPrimary::Pressure => Ok(0),
            CellPrimary::Composition(k) => {
                if k + 1 == self.component_count {
                    Err(LayoutError::DependentCompositionHasNoColumn {
                        index: k,
                        component_count: self.component_count,
                    })
                } else if k >= self.component_count {
                    Err(LayoutError::ComponentOutOfRange {
                        index: k,
                        component_count: self.component_count,
                    })
                } else {
                    Ok(1 + k)
                }
            }
        }
    }

    /// Local row of an equation within a cell block.
    pub fn local_row(&self, equation: CellEquation) -> Result<usize, LayoutError> {
        let CellEquation::ComponentBalance(i) = equation;
        if i >= self.component_count {
            Err(LayoutError::ComponentOutOfRange {
                index: i,
                component_count: self.component_count,
            })
        } else {
            Ok(i)
        }
    }

    /// Global matrix column of a cell's primary.
    pub fn cell_unknown(&self, cell: usize, primary: CellPrimary) -> Result<usize, LayoutError> {
        self.check_cell(cell)?;
        Ok(cell * self.cell_block_size() + self.local_column(primary)?)
    }

    /// Global matrix row of a cell's component balance.
    pub fn cell_equation(&self, cell: usize, equation: CellEquation) -> Result<usize, LayoutError> {
        self.check_cell(cell)?;
        Ok(cell * self.cell_block_size() + self.local_row(equation)?)
    }

    /// Invert [`Self::cell_unknown`]. `None` for a tail column.
    pub fn split_cell_unknown(&self, column: usize) -> Option<(usize, CellPrimary)> {
        let cell_columns = self.cell_count * self.cell_block_size();
        if column >= cell_columns {
            return None;
        }
        let cell = column / self.cell_block_size();
        let local = column % self.cell_block_size();
        let primary = if local == 0 {
            CellPrimary::Pressure
        } else {
            CellPrimary::Composition(local - 1)
        };
        Some((cell, primary))
    }

    /// Invert [`Self::cell_equation`]. `None` for a tail row.
    pub fn split_cell_equation(&self, row: usize) -> Option<(usize, CellEquation)> {
        let cell_rows = self.cell_count * self.cell_block_size();
        if row >= cell_rows {
            return None;
        }
        Some((
            row / self.cell_block_size(),
            CellEquation::ComponentBalance(row % self.cell_block_size()),
        ))
    }

    /// First column of the well-BHP tail.
    pub fn well_bhp_start(&self) -> usize {
        self.cell_count * self.cell_block_size()
    }

    /// Global column of a well's BHP unknown.
    pub fn well_bhp(&self, well: usize) -> Option<usize> {
        (well < self.well_count).then(|| self.well_bhp_start() + well)
    }

    /// First column of the perforation-rate tail.
    pub fn perforation_start(&self) -> usize {
        self.well_bhp_start() + self.well_count
    }

    /// Global column of a perforation's rate unknown.
    pub fn perforation(&self, perforation: usize) -> Option<usize> {
        (perforation < self.perforation_count).then(|| self.perforation_start() + perforation)
    }

    /// True when `column` is a cell's pressure column — what CPR restricts onto.
    pub fn is_cell_pressure_column(&self, column: usize) -> bool {
        matches!(
            self.split_cell_unknown(column),
            Some((_, CellPrimary::Pressure))
        )
    }

    /// The shared linear-solver metadata, so `fim/linear` sees one layout type whatever the
    /// model is.
    ///
    /// The plan requires the linear metadata to carry no water/oil/gas classification, and this is
    /// what makes that true: `FimLinearBlockLayout` describes block sizes and tail offsets and
    /// nothing about what the rows mean. The meaning stays here, where it is typed.
    pub fn to_linear_block_layout(&self) -> FimLinearBlockLayout {
        FimLinearBlockLayout {
            cell_block_count: self.cell_count,
            cell_block_size: self.cell_block_size(),
            well_bhp_count: self.well_count,
            perforation_tail_start: self.perforation_start(),
        }
    }

    fn check_cell(&self, cell: usize) -> Result<(), LayoutError> {
        if cell >= self.cell_count {
            Err(LayoutError::CellOutOfRange {
                index: cell,
                cell_count: self.cell_count,
            })
        } else {
            Ok(())
        }
    }
}
