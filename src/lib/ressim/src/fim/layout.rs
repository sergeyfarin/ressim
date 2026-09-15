//! `FIM-REPAIR-F7` (#22): the FIM row/column layout contract.
//!
//! Every FIM matrix in this crate interleaves each reservoir cell's unknowns and equations into a
//! contiguous block, then appends the well-BHP and perforation-rate tail. Until this module, the
//! block size and — more importantly — *what each row and column within a block means* lived as
//! bare `3`, `% 3` and `== 0` literals scattered across `assembly.rs`, `scaling.rs`,
//! `flow_lifecycle.rs` and the state/update code. Nothing named the contract, so nothing could
//! check it, and a reader had to infer a row's physics from arithmetic.
//!
//! The compositional plan ([`COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md`], C7) consumes
//! exactly this as its `EquationLayout`, with the explicit requirement: *"Do not infer a row's
//! physical meaning solely from `index % 3`."* That is the whole purpose of this module. It is
//! deliberately **only** the black-oil layout — no genericity over component count is introduced
//! here, because none is needed yet and the plan forbids making physics generic preemptively.
//!
//! This is a naming change, not a numerical one: every accessor computes the same arithmetic the
//! literals did.
//!
//! [`COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md`]: ../../../../docs/COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md

/// Primaries (equivalently, component equations) per reservoir cell in the black-oil model.
///
/// Seeds `FimLinearBlockLayout::cell_block_size`; prefer that field wherever a layout value is in
/// scope, so a reduced system (the well-Schur complement builds one) stays self-describing.
pub(crate) const CELL_BLOCK_SIZE: usize = 3;

/// A cell's primary unknowns, in matrix **column** order within the cell block.
///
/// The discriminants are the column offsets and are load-bearing — `assembly.rs` and the CPR
/// pressure restriction both rely on pressure being local column 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CellPrimary {
    /// Cell pressure [bar]. The CPR coarse system is built from exactly this column.
    Pressure = 0,
    /// Water saturation [-].
    WaterSaturation = 1,
    /// The hydrocarbon primary, whose meaning depends on the cell's `HydrocarbonState`: free-gas
    /// saturation when saturated, dissolved-gas ratio `Rs` when undersaturated. The column index
    /// is fixed; only the meaning switches. See `fim/state.rs`.
    Hydrocarbon = 2,
}

/// A cell's component conservation equations, in matrix **row** order within the cell block.
///
/// Named separately from [`CellPrimary`] on purpose: rows are equations and columns are unknowns,
/// and the fact that they currently share a block size is a property of this model, not a law.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CellEquation {
    /// Water component balance [Sm3].
    Water = 0,
    /// Oil component balance [Sm3].
    OilComponent = 1,
    /// Gas component balance [Sm3], free plus dissolved.
    GasComponent = 2,
}

impl CellPrimary {
    pub(crate) const ALL: [Self; CELL_BLOCK_SIZE] =
        [Self::Pressure, Self::WaterSaturation, Self::Hydrocarbon];

    pub(crate) const fn local_index(self) -> usize {
        self as usize
    }

    pub(crate) const fn from_local_index(local_var: usize) -> Option<Self> {
        match local_var {
            0 => Some(Self::Pressure),
            1 => Some(Self::WaterSaturation),
            2 => Some(Self::Hydrocarbon),
            _ => None,
        }
    }
}

impl CellEquation {
    pub(crate) const ALL: [Self; CELL_BLOCK_SIZE] =
        [Self::Water, Self::OilComponent, Self::GasComponent];

    pub(crate) const fn local_index(self) -> usize {
        self as usize
    }

    pub(crate) const fn from_local_index(local_eq: usize) -> Option<Self> {
        match local_eq {
            0 => Some(Self::Water),
            1 => Some(Self::OilComponent),
            2 => Some(Self::GasComponent),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The discriminants are indices into live matrices, so they are contract, not cosmetics.
    #[test]
    fn cell_primary_and_equation_indices_round_trip_in_block_order() {
        assert_eq!(CellPrimary::ALL.len(), CELL_BLOCK_SIZE);
        assert_eq!(CellEquation::ALL.len(), CELL_BLOCK_SIZE);

        for (expected, primary) in CellPrimary::ALL.iter().enumerate() {
            assert_eq!(primary.local_index(), expected);
            assert_eq!(CellPrimary::from_local_index(expected), Some(*primary));
        }
        for (expected, equation) in CellEquation::ALL.iter().enumerate() {
            assert_eq!(equation.local_index(), expected);
            assert_eq!(CellEquation::from_local_index(expected), Some(*equation));
        }

        assert_eq!(CellPrimary::from_local_index(CELL_BLOCK_SIZE), None);
        assert_eq!(CellEquation::from_local_index(CELL_BLOCK_SIZE), None);
    }

    /// Pressure must stay local column 0: `fim/linear`'s CPR restriction builds its coarse system
    /// from that column, and `flow_lifecycle.rs` still selects it with a literal `col % 3 == 0`.
    #[test]
    fn pressure_is_the_first_cell_column() {
        assert_eq!(CellPrimary::Pressure.local_index(), 0);
    }
}
