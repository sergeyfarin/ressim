//! C7 layout contract tests (`comp_layout_*`).

use super::layout::{
    CellEquation, CellPrimary, CompositionalLayout, LayoutError, MAX_COMPONENTS, MIN_COMPONENTS,
};
use crate::fim::layout::CELL_BLOCK_SIZE;

fn layout(n: usize, cells: usize) -> CompositionalLayout {
    CompositionalLayout::new(n, cells, 0, 0).unwrap()
}

/// Indexing must be a bijection between `(cell, primary)` and matrix columns, and between
/// `(cell, equation)` and rows. Checked for every supported component count, because an offset
/// written as `+ 2` instead of `+ (n - 1)` satisfies N=3 and nothing else.
#[test]
fn comp_layout_indexing_is_bijective_for_every_supported_component_count() {
    for n in MIN_COMPONENTS..=MAX_COMPONENTS {
        let l = layout(n, 7);
        let cell_columns = 7 * n;

        let mut seen_columns = vec![false; cell_columns];
        let mut seen_rows = vec![false; cell_columns];

        for cell in 0..7 {
            // Pressure plus the N-1 independent compositions.
            let mut primaries = vec![CellPrimary::Pressure];
            primaries.extend((0..n - 1).map(CellPrimary::Composition));
            assert_eq!(
                primaries.len(),
                n,
                "N={n}: block size is not the primary count"
            );

            for primary in primaries {
                let column = l.cell_unknown(cell, primary).unwrap();
                assert!(
                    column < cell_columns,
                    "N={n}: column {column} escaped the cell block"
                );
                assert!(!seen_columns[column], "N={n}: column {column} used twice");
                seen_columns[column] = true;
                assert_eq!(l.split_cell_unknown(column), Some((cell, primary)));
            }

            for i in 0..n {
                let equation = CellEquation::ComponentBalance(i);
                let row = l.cell_equation(cell, equation).unwrap();
                assert!(!seen_rows[row], "N={n}: row {row} used twice");
                seen_rows[row] = true;
                assert_eq!(l.split_cell_equation(row), Some((cell, equation)));
            }
        }

        assert!(
            seen_columns.iter().all(|&s| s),
            "N={n}: some column is unreachable"
        );
        assert!(
            seen_rows.iter().all(|&s| s),
            "N={n}: some row is unreachable"
        );
    }
}

/// The N=4 case the plan requires. Nothing in V1's pinned fluids uses four components — this
/// exists purely to expose an assumption that three components happen to satisfy.
#[test]
fn comp_layout_supports_a_synthetic_four_component_case() {
    let l = layout(4, 3);
    assert_eq!(l.cell_block_size(), 4);
    assert_eq!(l.independent_composition_count(), 3);

    // Cell 2's block starts at column 8, not at 6 — which is what a hard-coded 3 would give.
    assert_eq!(l.cell_unknown(2, CellPrimary::Pressure).unwrap(), 8);
    assert_eq!(l.cell_unknown(2, CellPrimary::Composition(2)).unwrap(), 11);
    assert_eq!(
        l.cell_equation(2, CellEquation::ComponentBalance(3))
            .unwrap(),
        11
    );

    // And component 3 is the dependent one, so it has a balance but no column.
    assert!(
        l.cell_equation(2, CellEquation::ComponentBalance(3))
            .is_ok()
    );
    assert!(matches!(
        l.cell_unknown(2, CellPrimary::Composition(3)),
        Err(LayoutError::DependentCompositionHasNoColumn { .. })
    ));
}

/// The dependent composition has a balance equation but no unknown, and the two failure modes are
/// distinguished: `N-1` is a real component with no column, `N` and beyond is not a component.
#[test]
fn comp_layout_dependent_composition_has_an_equation_but_no_column() {
    for n in MIN_COMPONENTS..=MAX_COMPONENTS {
        let l = layout(n, 2);
        assert!(matches!(
            l.cell_unknown(0, CellPrimary::Composition(n - 1)),
            Err(LayoutError::DependentCompositionHasNoColumn { .. })
        ));
        assert!(matches!(
            l.cell_unknown(0, CellPrimary::Composition(n)),
            Err(LayoutError::ComponentOutOfRange { .. })
        ));
        assert!(
            l.cell_equation(0, CellEquation::ComponentBalance(n - 1))
                .is_ok()
        );
        assert!(matches!(
            l.cell_equation(0, CellEquation::ComponentBalance(n)),
            Err(LayoutError::ComponentOutOfRange { .. })
        ));
    }
}

/// Equations and primaries must be equinumerous, or the system is over- or under-determined. This
/// is the plan's "exactly as many independent equations as primaries" stated as a test.
#[test]
fn comp_layout_has_as_many_equations_as_primaries() {
    for n in MIN_COMPONENTS..=MAX_COMPONENTS {
        let l = layout(n, 5);
        let primaries = 1 + l.independent_composition_count();
        let equations = n;
        assert_eq!(primaries, equations, "N={n}");
        assert_eq!(l.cell_block_size(), primaries);
    }
}

/// Pressure must be local column 0 for every component count: `fim/linear`'s CPR restriction
/// identifies it positionally, and the compositional model reuses that machinery.
#[test]
fn comp_layout_pressure_is_always_the_first_column() {
    for n in MIN_COMPONENTS..=MAX_COMPONENTS {
        let l = layout(n, 4);
        for cell in 0..4 {
            let column = l.cell_unknown(cell, CellPrimary::Pressure).unwrap();
            assert_eq!(column % l.cell_block_size(), 0);
            assert!(l.is_cell_pressure_column(column));
            for k in 0..(n - 1) {
                let other = l.cell_unknown(cell, CellPrimary::Composition(k)).unwrap();
                assert!(!l.is_cell_pressure_column(other));
            }
        }
    }
}

/// Tail offsets: well BHP unknowns follow every cell block, perforations follow those, and the
/// totals add up.
#[test]
fn comp_layout_tail_offsets_follow_the_cell_blocks() {
    let l = CompositionalLayout::new(3, 6, 2, 5).unwrap();
    assert_eq!(l.well_bhp_start(), 6 * 3);
    assert_eq!(l.well_bhp(0), Some(18));
    assert_eq!(l.well_bhp(1), Some(19));
    assert_eq!(l.well_bhp(2), None);
    assert_eq!(l.perforation_start(), 20);
    assert_eq!(l.perforation(4), Some(24));
    assert_eq!(l.perforation(5), None);
    assert_eq!(l.total_unknowns(), 18 + 2 + 5);

    // Splitting a tail column must return None, not a nonsensical cell.
    assert_eq!(l.split_cell_unknown(18), None);
    assert_eq!(l.split_cell_equation(24), None);
    assert!(!l.is_cell_pressure_column(18));
}

/// The shared linear metadata must describe the same partition, so `fim/linear` needs no
/// compositional special case — and must carry no phase classification of its own.
#[test]
fn comp_layout_agrees_with_the_shared_linear_block_layout() {
    for n in MIN_COMPONENTS..=MAX_COMPONENTS {
        let l = CompositionalLayout::new(n, 6, 2, 5).unwrap();
        let shared = l.to_linear_block_layout();

        assert_eq!(shared.cell_block_size, n);
        assert_eq!(shared.cell_block_count, 6);
        assert_eq!(shared.cell_unknown_count(), 6 * n);
        assert_eq!(shared.well_bhp_start(), l.well_bhp_start());
        assert_eq!(shared.well_bhp_end(), l.perforation_start());
        assert_eq!(shared.perforation_tail_start, l.perforation_start());

        // Every cell column must be identified the same way by both.
        for column in 0..(6 * n) {
            assert_eq!(
                shared.is_cell_pressure_column(column),
                l.is_cell_pressure_column(column),
                "N={n}, column {column}"
            );
            let (cell, local) = shared.split_cell_unknown(column).unwrap();
            let (our_cell, our_primary) = l.split_cell_unknown(column).unwrap();
            assert_eq!(cell, our_cell);
            assert_eq!(l.local_column(our_primary).unwrap(), local);
        }
    }
}

/// `FimLinearBlockLayout::is_cell_pressure_column` is implemented against the *black-oil*
/// `CellPrimary::Pressure` index. That is only correct for the compositional model because both
/// put pressure at local 0, and this test is what makes that coincidence a checked one.
#[test]
fn comp_layout_shares_the_pressure_column_convention_with_black_oil() {
    assert_eq!(crate::fim::layout::CellPrimary::Pressure.local_index(), 0);
    let l = layout(3, 2);
    assert_eq!(l.local_column(CellPrimary::Pressure).unwrap(), 0);
}

/// The black-oil layout is untouched. The two models have separate layouts because they have
/// different equations, and an accidental change to the shared constant would silently move every
/// existing FIM matrix row.
#[test]
fn comp_layout_leaves_the_black_oil_block_size_alone() {
    assert_eq!(CELL_BLOCK_SIZE, 3);
}

#[test]
fn comp_layout_rejects_unsupported_component_counts() {
    for n in [0usize, 1, MAX_COMPONENTS + 1, 10] {
        assert!(matches!(
            CompositionalLayout::new(n, 4, 0, 0),
            Err(LayoutError::UnsupportedComponentCount { .. })
        ));
    }
}

#[test]
fn comp_layout_rejects_an_out_of_range_cell() {
    let l = layout(3, 2);
    assert!(matches!(
        l.cell_unknown(2, CellPrimary::Pressure),
        Err(LayoutError::CellOutOfRange { .. })
    ));
    assert!(matches!(
        l.cell_equation(9, CellEquation::ComponentBalance(0)),
        Err(LayoutError::CellOutOfRange { .. })
    ));
}

/// An empty grid is a degenerate but representable layout: zero cells, and the tail starts at
/// zero. Worth pinning because an off-by-one here would only show up on a grid nobody tests.
#[test]
fn comp_layout_handles_an_empty_grid() {
    let l = CompositionalLayout::new(3, 0, 1, 0).unwrap();
    assert_eq!(l.well_bhp_start(), 0);
    assert_eq!(l.well_bhp(0), Some(0));
    assert_eq!(l.split_cell_unknown(0), None);
    assert_eq!(l.total_unknowns(), 1);
}
