//! C9 global assembly contract tests (`comp_assembly_*`).

use super::accumulation::cell_inventory;
use super::assembly::{AssemblyError, Face, assemble, numerical_jacobian};
use super::flux::{Gravity, HydrocarbonRelPerm};
use super::layout::CompositionalLayout;
use super::state::{CompositionalCellState, CompositionalState, RockView};
use crate::fluid::pinned;
use crate::fluid::specification::FluidSpecification;

const RELPERM: HydrocarbonRelPerm = HydrocarbonRelPerm::StraightLine;
const GEOM_T: f64 = 8.526_988_8e-3 * 100.0 * (100.0 * 10.0) / 100.0;

struct Grid {
    spec: FluidSpecification,
    layout: CompositionalLayout,
    pore_volumes: Vec<f64>,
    faces: Vec<Face>,
    state: CompositionalState,
}

impl Grid {
    fn rock(&self) -> RockView<'_> {
        RockView {
            pore_volume_ref_m3: &self.pore_volumes,
            reference_pressure_bar: 200.0,
            compressibility_per_bar: 4.0e-5,
        }
    }
}

/// A 1D column of `cells`, each with a linear pressure gradient and a common composition.
fn column(spec: FluidSpecification, cells: usize, p_high: f64, p_low: f64, z: &[f64]) -> Grid {
    let layout = CompositionalLayout::new(spec.component_count(), cells, 0, 0).unwrap();
    let step = if cells > 1 {
        (p_high - p_low) / (cells - 1) as f64
    } else {
        0.0
    };
    let state = CompositionalState::new(
        &layout,
        (0..cells)
            .map(|i| CompositionalCellState::new(p_high - step * i as f64, z.to_vec()).unwrap())
            .collect(),
    )
    .unwrap();
    Grid {
        spec,
        layout,
        pore_volumes: vec![1000.0; cells],
        faces: (0..cells.saturating_sub(1))
            .map(|i| Face {
                cell_i: i,
                cell_j: i + 1,
                geom_t: GEOM_T,
                gravity: Gravity::OFF,
            })
            .collect(),
        state,
    }
}

/// The previous inventory that makes the current state stationary.
fn stationary_previous(grid: &Grid) -> Vec<Vec<f64>> {
    let rock = grid.rock();
    (0..grid.layout.cell_count())
        .map(|c| {
            cell_inventory(&grid.spec, &rock, c, grid.state.cell(c))
                .unwrap()
                .0
                .component_moles
        })
        .collect()
}

fn zero_sources(grid: &Grid) -> Vec<Vec<f64>> {
    vec![vec![0.0; grid.layout.component_count()]; grid.layout.cell_count()]
}

// ---------------------------------------------------------------------------------------------
// Conservation
// ---------------------------------------------------------------------------------------------

/// A closed grid conserves every component exactly. The residual summed over all cells must equal
/// the inventory change alone — faces contribute nothing to the total, whatever they carry
/// internally.
#[test]
fn comp_assembly_internal_faces_cancel_globally() {
    let grid = column(pinned::ternary().unwrap(), 5, 200.0, 150.0, &[0.2, 0.5]);
    let rock = grid.rock();
    let previous = stationary_previous(&grid);

    let result = assemble(
        &grid.spec,
        &grid.layout,
        &rock,
        RELPERM,
        &grid.state,
        &grid.faces,
        &previous,
        &zero_sources(&grid),
        1.0,
    )
    .unwrap();

    for i in 0..3 {
        assert_eq!(
            result.net_internal_face_moles[i], 0.0,
            "component {i}: internal faces do not cancel globally"
        );
    }

    // The previous inventory was taken at this same state, so the accumulation part is zero and
    // the whole residual is the face transport — which sums to zero.
    let n = 3;
    for i in 0..n {
        let total: f64 = (0..5).map(|c| result.residual[c * n + i]).sum();
        let scale: f64 = (0..5)
            .map(|c| result.residual[c * n + i].abs())
            .fold(0.0, f64::max)
            .max(1.0);
        assert!(
            total.abs() / scale < 1e-12,
            "component {i}: the closed grid's residual sums to {total}"
        );
    }
}

/// A uniform state is stationary: no pressure gradient, no flux, no residual anywhere.
#[test]
fn comp_assembly_a_uniform_closed_grid_is_stationary() {
    for (spec, z) in [
        (pinned::binary().unwrap(), vec![0.6]),
        (pinned::ternary().unwrap(), vec![0.2, 0.5]),
    ] {
        let n = spec.component_count();
        let grid = column(spec, 4, 150.0, 150.0, &z);
        let rock = grid.rock();
        let previous = stationary_previous(&grid);

        let result = assemble(
            &grid.spec,
            &grid.layout,
            &rock,
            RELPERM,
            &grid.state,
            &grid.faces,
            &previous,
            &zero_sources(&grid),
            5.0,
        )
        .unwrap();

        for (index, r) in result.residual.iter().enumerate() {
            assert_eq!(
                *r, 0.0,
                "N={n}: entry {index} is nonzero in a uniform closed grid"
            );
        }
        assert_eq!(result.scaled_residual_norm(&grid.layout), 0.0);
    }
}

/// Sources are the only way material enters or leaves a closed grid, and they are accounted
/// exactly: the total residual equals minus `dt` times the total source.
#[test]
fn comp_assembly_a_source_is_the_only_way_material_enters() {
    let grid = column(pinned::ternary().unwrap(), 4, 150.0, 150.0, &[0.2, 0.5]);
    let rock = grid.rock();
    let previous = stationary_previous(&grid);

    let mut sources = zero_sources(&grid);
    sources[0] = vec![100.0, 200.0, 50.0]; // injection into the first cell
    sources[3] = vec![-40.0, -80.0, -20.0]; // production from the last

    let dt = 3.0;
    let result = assemble(
        &grid.spec,
        &grid.layout,
        &rock,
        RELPERM,
        &grid.state,
        &grid.faces,
        &previous,
        &sources,
        dt,
    )
    .unwrap();

    for i in 0..3 {
        let total: f64 = (0..4).map(|c| result.residual[c * 3 + i]).sum();
        let expected = -dt * (sources[0][i] + sources[3][i]);
        assert!(
            (total - expected).abs() < 1e-9 * expected.abs().max(1.0),
            "component {i}: residual sums to {total}, expected {expected}"
        );
    }
}

/// A 1D column with a pressure gradient moves material downhill, and the interior cells pass it
/// along: only the ends gain or lose.
#[test]
fn comp_assembly_a_1d_column_transports_from_the_high_pressure_end() {
    let grid = column(pinned::ternary().unwrap(), 5, 200.0, 150.0, &[0.2, 0.5]);
    let rock = grid.rock();
    let previous = stationary_previous(&grid);

    let result = assemble(
        &grid.spec,
        &grid.layout,
        &rock,
        RELPERM,
        &grid.state,
        &grid.faces,
        &previous,
        &zero_sources(&grid),
        1.0,
    )
    .unwrap();

    // The first cell has an outgoing face only, so its residual is positive (it loses material).
    // The last has an incoming face only, so its residual is negative.
    for i in 0..3 {
        assert!(
            result.residual[i] > 0.0,
            "component {i}: the high-pressure end should be losing material"
        );
        assert!(
            result.residual[4 * 3 + i] < 0.0,
            "component {i}: the low-pressure end should be gaining material"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The Jacobian
// ---------------------------------------------------------------------------------------------

/// Every entry of the assembled Jacobian against an independent numerical one, built by
/// perturbing the state and re-running the whole assembler. No helper is shared between the two
/// paths, so an error in one cannot cancel an error in the other.
#[test]
fn comp_assembly_jacobian_matches_a_numerical_jacobian_entrywise() {
    let cases: Vec<(&str, FluidSpecification, usize, f64, f64, Vec<f64>)> = vec![
        (
            "binary 2-cell",
            pinned::binary().unwrap(),
            2,
            160.0,
            140.0,
            vec![0.6],
        ),
        (
            "binary 3-cell",
            pinned::binary().unwrap(),
            3,
            400.0,
            380.0,
            vec![0.6],
        ),
        (
            "ternary 3-cell",
            pinned::ternary().unwrap(),
            3,
            170.0,
            150.0,
            vec![0.2, 0.5],
        ),
    ];

    let mut worst = (0.0f64, String::new());

    for (name, spec, cells, p_high, p_low, z) in cases {
        let n = spec.component_count();
        let grid = column(spec, cells, p_high, p_low, &z);
        let rock = grid.rock();
        let previous = stationary_previous(&grid);
        let sources = zero_sources(&grid);
        let dt = 1.0;

        let analytic = assemble(
            &grid.spec,
            &grid.layout,
            &rock,
            RELPERM,
            &grid.state,
            &grid.faces,
            &previous,
            &sources,
            dt,
        )
        .unwrap();

        let numerical = numerical_jacobian(
            &grid.spec,
            &grid.layout,
            &rock,
            RELPERM,
            &grid.state,
            &grid.faces,
            &previous,
            &sources,
            dt,
            1e-3,
            1e-6,
        )
        .unwrap();

        // Scale each column by its own largest entry, so a small column is not held to the
        // magnitude of a large one — the plan's "scaled entrywise" comparison.
        let size = cells * n;
        for column_index in 0..size {
            let scale = (0..size)
                .map(|r| numerical[r][column_index].abs())
                .fold(0.0f64, f64::max)
                .max(1e-8);
            for row in 0..size {
                let e = (analytic.jacobian[row][column_index] - numerical[row][column_index]).abs()
                    / scale;
                if e > worst.0 {
                    worst = (e, format!("{name}[{row}][{column_index}]"));
                }
            }
        }
    }

    assert!(
        worst.0 < 1e-5,
        "the assembled Jacobian disagrees with the numerical one: worst scaled error {:e} at {}",
        worst.0,
        worst.1
    );
}

/// The sparsity is the grid's: a cell couples to itself and to its face neighbours, and to nothing
/// else. A dense-looking Jacobian would still pass the entrywise comparison while making C10's
/// sparse assembly impossible.
#[test]
fn comp_assembly_jacobian_couples_only_face_neighbours() {
    let grid = column(pinned::ternary().unwrap(), 4, 200.0, 150.0, &[0.2, 0.5]);
    let rock = grid.rock();
    let previous = stationary_previous(&grid);

    let result = assemble(
        &grid.spec,
        &grid.layout,
        &rock,
        RELPERM,
        &grid.state,
        &grid.faces,
        &previous,
        &zero_sources(&grid),
        1.0,
    )
    .unwrap();

    let n = 3;
    for row in 0..(4 * n) {
        let (row_cell, _) = grid.layout.split_cell_equation(row).unwrap();
        for col in 0..(4 * n) {
            let (col_cell, _) = grid.layout.split_cell_unknown(col).unwrap();
            let adjacent = row_cell == col_cell
                || grid.faces.iter().any(|f| {
                    (f.cell_i, f.cell_j) == (row_cell, col_cell)
                        || (f.cell_j, f.cell_i) == (row_cell, col_cell)
                });
            if !adjacent {
                assert_eq!(
                    result.jacobian[row][col], 0.0,
                    "cells {row_cell} and {col_cell} are not connected but entry \
                     [{row}][{col}] is nonzero"
                );
            }
        }
    }
}

/// Repeated assembly of the same state gives bit-identical results. Anything that mutated a cache
/// or accumulated into a shared buffer would show up here rather than as a drift someone
/// eventually notices.
#[test]
fn comp_assembly_is_deterministic_and_mutates_nothing() {
    let grid = column(pinned::ternary().unwrap(), 4, 200.0, 150.0, &[0.2, 0.5]);
    let rock = grid.rock();
    let previous = stationary_previous(&grid);
    let sources = zero_sources(&grid);
    let before = grid.state.clone();

    let a = assemble(
        &grid.spec,
        &grid.layout,
        &rock,
        RELPERM,
        &grid.state,
        &grid.faces,
        &previous,
        &sources,
        1.0,
    )
    .unwrap();
    let b = assemble(
        &grid.spec,
        &grid.layout,
        &rock,
        RELPERM,
        &grid.state,
        &grid.faces,
        &previous,
        &sources,
        1.0,
    )
    .unwrap();

    assert_eq!(a, b);
    assert_eq!(
        grid.state, before,
        "assembly mutated the state it was given"
    );
}

/// Scaling is per cell and is carried out of the assembler, so a convergence test does not have to
/// rebuild it.
#[test]
fn comp_assembly_reports_per_cell_scaling() {
    let grid = column(pinned::ternary().unwrap(), 3, 200.0, 150.0, &[0.2, 0.5]);
    let rock = grid.rock();
    let previous = stationary_previous(&grid);

    let result = assemble(
        &grid.spec,
        &grid.layout,
        &rock,
        RELPERM,
        &grid.state,
        &grid.faces,
        &previous,
        &zero_sources(&grid),
        1.0,
    )
    .unwrap();

    assert_eq!(result.scaling.len(), 3);
    assert_eq!(result.inventories.len(), 3);
    for c in 0..3 {
        assert_eq!(result.scaling[c].row_scale, previous[c]);
        assert_eq!(
            result.scaling[c].primary_scale[0],
            grid.state.cell(c).pressure_bar
        );
    }
    // The norm is finite and positive: this grid has a gradient, so it is not stationary.
    let norm = result.scaled_residual_norm(&grid.layout);
    assert!(norm > 0.0 && norm.is_finite(), "scaled norm {norm}");
}

#[test]
fn comp_assembly_rejects_malformed_input() {
    let grid = column(pinned::binary().unwrap(), 2, 160.0, 150.0, &[0.6]);
    let rock = grid.rock();
    let previous = stationary_previous(&grid);
    let sources = zero_sources(&grid);

    let bad_face = [Face {
        cell_i: 0,
        cell_j: 5,
        geom_t: GEOM_T,
        gravity: Gravity::OFF,
    }];
    assert!(matches!(
        assemble(
            &grid.spec,
            &grid.layout,
            &rock,
            RELPERM,
            &grid.state,
            &bad_face,
            &previous,
            &sources,
            1.0
        ),
        Err(AssemblyError::InvalidFace { .. })
    ));

    let self_face = [Face {
        cell_i: 1,
        cell_j: 1,
        geom_t: GEOM_T,
        gravity: Gravity::OFF,
    }];
    assert!(matches!(
        assemble(
            &grid.spec,
            &grid.layout,
            &rock,
            RELPERM,
            &grid.state,
            &self_face,
            &previous,
            &sources,
            1.0
        ),
        Err(AssemblyError::InvalidFace { .. })
    ));

    assert!(matches!(
        assemble(
            &grid.spec,
            &grid.layout,
            &rock,
            RELPERM,
            &grid.state,
            &grid.faces,
            &previous[..1],
            &sources,
            1.0
        ),
        Err(AssemblyError::ShapeMismatch {
            what: "previous inventory rows",
            ..
        })
    ));
    assert!(matches!(
        assemble(
            &grid.spec,
            &grid.layout,
            &rock,
            RELPERM,
            &grid.state,
            &grid.faces,
            &previous,
            &sources[..1],
            1.0
        ),
        Err(AssemblyError::ShapeMismatch {
            what: "source rows",
            ..
        })
    ));
}
