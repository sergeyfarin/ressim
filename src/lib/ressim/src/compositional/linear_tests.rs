//! Block-sparse storage and the linear routes (`comp_linear_*`, #49).
//!
//! The dense LU is the oracle. Sparse LU must agree with it to roundoff; FGMRES-CPR must meet its
//! own tolerance on the **true** full-system residual and land within what that tolerance allows
//! of the oracle's correction. Every route's report must carry the same observables, measured the
//! same way, or a comparison between them is not admissible (`COMPOSITIONAL_VALIDATION.md` §6).
//!
//! The SPE5-size grid is SPE5's own 7x7x3 at its six components (882 rows). The SPE3-size grid is
//! SPE3's 9x9x4 on `cost_tests`' ten-component **timing fluid** (3,240 rows): SPE3's own fluid is
//! not pinned, and the timing fluid is not validated and must never become a case. It is here
//! because it has SPE3's row count and block size, which is what a linear solver's cost depends
//! on.

use std::time::Instant;

use super::accumulation::cell_inventory;
use super::assembly::{AssemblyResult, Face, assemble};
use super::block_matrix::BlockSparseMatrix;
use super::cost_tests::{TEN_Z, ten_component_fluid};
use super::flux::Gravity;
use super::layout::CompositionalLayout;
use super::linear::{
    LinearFailure, LinearOptions, LinearRoute, LinearSolveReport, pressure_weights, solve,
};
use super::newton::{NewtonError, NewtonOptions, NewtonProblem, solve_newton};
use super::relperm::RelativePermeabilityModel;
use super::state::{CompositionalCellState, CompositionalState, RockView};
use super::wells::{CompositionalWell, InjectedFluid, WellControl};
use crate::fluid::pinned;
use crate::fluid::specification::FluidSpecification;

const GEOM_T: f64 = 8.526_988_8e-3 * 100.0 * (100.0 * 10.0) / 100.0;
const SPE5_Z: [f64; 6] = [0.50, 0.03, 0.07, 0.20, 0.15, 0.05];

fn relperm() -> RelativePermeabilityModel {
    RelativePermeabilityModel::Linear
}

/// A box grid with a pressure gradient in every direction, gravity on the vertical faces, a
/// previous inventory 1.5 bar above the current state (so the accumulation residual is not zero),
/// and a BHP producer and injector at opposite corners.
struct BoxGrid {
    spec: FluidSpecification,
    layout: CompositionalLayout,
    pore_volumes: Vec<f64>,
    faces: Vec<Face>,
    state: CompositionalState,
    previous: Vec<Vec<f64>>,
    sources: Vec<Vec<f64>>,
    wells: Vec<CompositionalWell>,
}

impl BoxGrid {
    fn new(spec: FluidSpecification, z: &[f64], nx: usize, ny: usize, nz: usize) -> Self {
        let n = spec.component_count();
        let cells = nx * ny * nz;
        let index = |i: usize, j: usize, k: usize| (k * ny + j) * nx + i;
        let depth = |k: usize| 2000.0 + 5.0 * k as f64;
        let pressure = |i: usize, j: usize, k: usize| {
            130.0 + 0.4 * i as f64 - 0.25 * j as f64 + 0.6 * k as f64
        };

        let mut faces = Vec::new();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let here = index(i, j, k);
                    if i + 1 < nx {
                        faces.push(face(here, index(i + 1, j, k), Gravity::OFF));
                    }
                    if j + 1 < ny {
                        faces.push(face(here, index(i, j + 1, k), Gravity::OFF));
                    }
                    if k + 1 < nz {
                        let g = Gravity::between(depth(k), depth(k + 1));
                        faces.push(face(here, index(i, j, k + 1), g));
                    }
                }
            }
        }

        let layout = CompositionalLayout::new(n, cells, 0, 0).unwrap();
        let mut current = vec![None; cells];
        let mut before = vec![None; cells];
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let p = pressure(i, j, k);
                    let independent = z[..n - 1].to_vec();
                    current[index(i, j, k)] =
                        Some(CompositionalCellState::new(p, independent.clone()).unwrap());
                    before[index(i, j, k)] =
                        Some(CompositionalCellState::new(p + 1.5, independent).unwrap());
                }
            }
        }
        let state =
            CompositionalState::new(&layout, current.into_iter().map(Option::unwrap).collect())
                .unwrap();
        let pore_volumes = vec![1000.0; cells];
        let rock = RockView {
            pore_volume_ref_m3: &pore_volumes,
            reference_pressure_bar: 200.0,
            compressibility_per_bar: 4.0e-5,
        };
        let previous = before
            .into_iter()
            .enumerate()
            .map(|(c, s)| {
                cell_inventory(&spec, &rock, c, &s.unwrap())
                    .unwrap()
                    .0
                    .component_moles
            })
            .collect();

        let producer_cell = index(nx - 1, ny - 1, 0);
        let injector_cell = index(0, 0, nz - 1);
        let mut injection = vec![0.0; n];
        injection[0] = 0.9;
        injection[1] = 0.1;
        let wells = vec![
            CompositionalWell::single(
                "PROD",
                producer_cell,
                5.0,
                WellControl::Bhp {
                    target_bar: pressure(nx - 1, ny - 1, 0) - 15.0,
                },
            ),
            CompositionalWell {
                injection: Some(InjectedFluid::Hydrocarbon(injection)),
                ..CompositionalWell::single(
                    "INJ",
                    injector_cell,
                    5.0,
                    WellControl::Bhp {
                        target_bar: pressure(0, 0, nz - 1) + 15.0,
                    },
                )
            },
        ];

        Self {
            spec,
            layout,
            pore_volumes,
            faces,
            state,
            previous,
            sources: vec![vec![0.0; n]; cells],
            wells,
        }
    }

    fn rock(&self) -> RockView<'_> {
        RockView {
            pore_volume_ref_m3: &self.pore_volumes,
            reference_pressure_bar: 200.0,
            compressibility_per_bar: 4.0e-5,
        }
    }

    fn assemble(&self) -> AssemblyResult {
        assemble(
            &self.spec,
            &self.layout,
            &self.rock(),
            &relperm(),
            &self.state,
            &self.faces,
            &self.wells,
            &self.previous,
            &self.sources,
            1.0,
        )
        .unwrap()
    }

    /// The row-scaled system exactly as Newton hands it to the linear solve.
    fn scaled_system(&self) -> (BlockSparseMatrix, Vec<Vec<f64>>, Vec<f64>) {
        let assembled = self.assemble();
        let n = self.layout.component_count();
        let scale: Vec<f64> = assembled
            .scaling
            .iter()
            .flat_map(|s| s.row_scale.iter().copied())
            .collect();
        let mut matrix = assembled.jacobian;
        matrix.scale_rows(&scale);
        let blocks = assembled
            .accumulation_blocks
            .iter()
            .enumerate()
            .map(|(cell, block)| {
                let mut b = block.clone();
                for i in 0..n {
                    for v in &mut b[i * n..(i + 1) * n] {
                        *v /= scale[cell * n + i];
                    }
                }
                b
            })
            .collect();
        let rhs = assembled
            .residual
            .iter()
            .zip(&scale)
            .map(|(r, s)| -r / s)
            .collect();
        (matrix, blocks, rhs)
    }
}

fn face(cell_i: usize, cell_j: usize, gravity: Gravity) -> Face {
    Face {
        cell_i,
        cell_j,
        geom_t: GEOM_T,
        gravity,
    }
}

fn route(route: LinearRoute) -> LinearOptions {
    LinearOptions {
        route,
        ..LinearOptions::default()
    }
}

fn max_relative_difference(a: &[f64], oracle: &[f64]) -> f64 {
    let scale = oracle.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let diff = a
        .iter()
        .zip(oracle)
        .fold(0.0f64, |m, (x, y)| m.max((x - y).abs()));
    diff / scale
}

/// The comparable observables every route must carry (hard rule 5).
fn assert_comparable(report: &LinearSolveReport, rhs_norm: f64) {
    assert_eq!(report.rhs_norm, rhs_norm, "{report:?}");
    assert!(report.finite, "{report:?}");
    assert!(report.residual_norm.is_finite(), "{report:?}");
    assert!(report.reduction().is_finite(), "{report:?}");
}

#[test]
fn comp_linear_block_pattern_is_the_face_graph_plus_the_diagonal() {
    // A repeated face and both orientations of one share a block.
    let m = BlockSparseMatrix::from_connections(4, 2, [(0, 1), (1, 0), (1, 2), (0, 1)]).unwrap();
    assert_eq!(m.rows(), 8);
    assert_eq!(m.stored_blocks(), 4 + 2 * 2);
    for (i, j) in [
        (0, 0),
        (0, 1),
        (1, 0),
        (1, 1),
        (1, 2),
        (2, 1),
        (2, 2),
        (3, 3),
    ] {
        assert!(m.position(i, j).is_some(), "({i},{j}) missing");
    }
    for (i, j) in [(0, 2), (2, 0), (0, 3), (3, 2)] {
        assert!(m.position(i, j).is_none(), "({i},{j}) should not be stored");
    }
    assert!(BlockSparseMatrix::from_connections(3, 2, [(0, 3)]).is_none());
    assert!(BlockSparseMatrix::from_connections(3, 2, [(1, 1)]).is_none());
}

#[test]
fn comp_linear_block_matrix_multiply_and_dense_agree() {
    let mut m = BlockSparseMatrix::from_connections(3, 2, [(0, 1), (1, 2)]).unwrap();
    for position in 0..m.stored_blocks() {
        for (k, v) in m.block_mut(position).iter_mut().enumerate() {
            *v = 1.0 + position as f64 + 0.25 * k as f64;
        }
    }
    let dense = m.to_dense();
    let x = [1.0, -2.0, 0.5, 3.0, -1.5, 2.5];
    let mut y = [0.0; 6];
    m.multiply(&x, &mut y);
    for r in 0..6 {
        let expected: f64 = (0..6).map(|c| dense[r][c] * x[c]).sum();
        assert_eq!(y[r], expected, "row {r}");
        for c in 0..6 {
            assert_eq!(m.get(r, c), dense[r][c]);
        }
    }
    assert_eq!(m.triplets().count(), m.stored_blocks() * 4);

    m.scale_rows(&[2.0, 4.0, 1.0, 1.0, 0.5, 8.0]);
    let scaled = m.to_dense();
    assert_eq!(scaled[0][1], dense[0][1] / 2.0);
    assert_eq!(scaled[5][4], dense[5][4] / 8.0);
}

/// The assembled pattern is the box grid's face graph: every cell has itself and its up-to-six
/// neighbours, and nothing else is stored.
#[test]
fn comp_linear_assembled_pattern_stores_only_face_neighbours() {
    let grid = BoxGrid::new(pinned::spe5().unwrap(), &SPE5_Z, 3, 3, 2);
    let a = grid.assemble();
    let cells = grid.layout.cell_count();
    assert_eq!(a.jacobian.block_rows(), cells);
    assert_eq!(a.jacobian.stored_blocks(), cells + 2 * grid.faces.len());
    assert_eq!(a.accumulation_blocks.len(), cells);
}

/// The CPR weights' defining property, on SPE5's own two-phase cells: the weighted sum of a cell's
/// component accumulation derivatives is one in pressure and zero in every composition.
#[test]
fn comp_linear_pressure_weights_annihilate_the_composition_columns() {
    let grid = BoxGrid::new(pinned::spe5().unwrap(), &SPE5_Z, 3, 2, 1);
    let (_, blocks, _) = grid.scaled_system();
    let n = grid.layout.component_count();
    for (cell, block) in blocks.iter().enumerate() {
        let w = pressure_weights(block, n).expect("a compressible cell has weights");
        for v in 0..n {
            let column: f64 = (0..n).map(|i| w[i] * block[i * n + v]).sum();
            let expected = if v == 0 { 1.0 } else { 0.0 };
            assert!(
                (column - expected).abs() < 1e-10,
                "cell {cell}, column {v}: {column:e}"
            );
        }
    }
}

/// A singular accumulation block has no weights, and the CPR build names it rather than
/// proceeding on a garbage pressure equation.
#[test]
fn comp_linear_cpr_rejects_a_singular_accumulation_block() {
    assert!(pressure_weights(&[1.0, 2.0, 2.0, 4.0], 2).is_none());

    let grid = BoxGrid::new(pinned::spe5().unwrap(), &SPE5_Z, 2, 1, 1);
    let (matrix, mut blocks, rhs) = grid.scaled_system();
    blocks[1].fill(0.0);
    let (outcome, report) = solve(&route(LinearRoute::FgmresCpr), &matrix, &blocks, &rhs);
    assert_eq!(
        outcome,
        Err(LinearFailure::Preconditioner {
            cell: 1,
            stage: "pressure-weight accumulation"
        })
    );
    assert!(!report.finite);
}

/// Every route on one moderately sized system: sparse LU agrees with the oracle to roundoff, CPR
/// meets its tolerance on the true residual and lands within what that tolerance allows, and all
/// three reports carry the same RHS norm.
#[test]
fn comp_linear_routes_agree_with_the_dense_oracle() {
    let grid = BoxGrid::new(pinned::spe5().unwrap(), &SPE5_Z, 4, 3, 2);
    let (matrix, blocks, rhs) = grid.scaled_system();
    assert_eq!(matrix.rows(), 144);

    let (dense, dense_report) = solve(&route(LinearRoute::DenseLu), &matrix, &blocks, &rhs);
    let dense = dense.unwrap();
    let (sparse, sparse_report) = solve(&route(LinearRoute::SparseLu), &matrix, &blocks, &rhs);
    let sparse = sparse.unwrap();
    let cpr_options = route(LinearRoute::FgmresCpr);
    let (cpr, cpr_report) = solve(&cpr_options, &matrix, &blocks, &rhs);
    let cpr = cpr.unwrap();

    let rhs_norm = dense_report.rhs_norm;
    assert!(rhs_norm > 0.0);
    for report in [&dense_report, &sparse_report, &cpr_report] {
        assert_comparable(report, rhs_norm);
    }
    // The acceptance matrix's "tiny direct linear oracle" row: <= 1e-10 full-system relative
    // residual, on both direct routes.
    assert!(dense_report.reduction() <= 1e-10, "{dense_report:?}");
    assert!(sparse_report.reduction() <= 1e-10, "{sparse_report:?}");
    assert!(dense_report.min_pivot.is_some());
    assert!(sparse_report.min_pivot.is_none());

    // 144 rows is under the direct threshold, so the default is sparse LU, and says so.
    let (_, auto_report) = solve(&LinearOptions::default(), &matrix, &blocks, &rhs);
    if std::env::var_os("RESSIM_COMP_LINEAR_ROUTE").is_none() {
        assert_eq!(auto_report.route, LinearRoute::SparseLu);
    }

    let sparse_gap = max_relative_difference(&sparse, &dense);
    assert!(sparse_gap <= 1e-10, "sparse vs dense: {sparse_gap:e}");

    assert!(
        cpr_report.reduction() <= cpr_options.relative_tolerance,
        "{cpr_report:?}"
    );
    assert!(cpr_report.iterations > 0);
    let cpr_gap = max_relative_difference(&cpr, &dense);
    assert!(cpr_gap <= 1e-7, "cpr vs dense: {cpr_gap:e}");
}

/// CPR that runs out of budget reports a typed failure carrying the true residual, never a
/// correction it did not converge.
#[test]
fn comp_linear_cpr_out_of_budget_is_a_failure_not_a_correction() {
    let grid = BoxGrid::new(pinned::spe5().unwrap(), &SPE5_Z, 4, 3, 2);
    let (matrix, blocks, rhs) = grid.scaled_system();
    let options = LinearOptions {
        route: LinearRoute::FgmresCpr,
        relative_tolerance: 1e-30,
        restart: 2,
        max_iterations: 2,
    };
    let (outcome, report) = solve(&options, &matrix, &blocks, &rhs);
    match outcome {
        Err(LinearFailure::NotConverged {
            iterations,
            relative_residual,
        }) => {
            assert_eq!(iterations, 2);
            assert_eq!(relative_residual, report.reduction());
            assert!(relative_residual > 0.0);
        }
        other => panic!("expected NotConverged, got {other:?}"),
    }
    assert!(report.finite);
}

/// SPE5's own grid size. Dense is not run: it would be the cost this route exists to avoid.
#[test]
fn comp_linear_spe5_size_sparse_and_cpr_agree() {
    let grid = BoxGrid::new(pinned::spe5().unwrap(), &SPE5_Z, 7, 7, 3);
    let (matrix, blocks, rhs) = grid.scaled_system();
    assert_eq!(matrix.rows(), 882);

    let (sparse, sparse_report) = solve(&route(LinearRoute::SparseLu), &matrix, &blocks, &rhs);
    let options = route(LinearRoute::FgmresCpr);
    let (cpr, cpr_report) = solve(&options, &matrix, &blocks, &rhs);
    let sparse = sparse.unwrap();
    let cpr = cpr.unwrap();
    assert_comparable(&sparse_report, sparse_report.rhs_norm);
    assert_comparable(&cpr_report, sparse_report.rhs_norm);
    assert!(sparse_report.reduction() <= 1e-10, "{sparse_report:?}");
    assert!(cpr_report.reduction() <= options.relative_tolerance);
    let gap = max_relative_difference(&cpr, &sparse);
    assert!(gap <= 1e-7, "cpr vs sparse: {gap:e}");

    // Above the direct threshold the default is CPR, and the report names it rather than "auto".
    let auto = LinearOptions {
        route: LinearRoute::Auto,
        ..LinearOptions::default()
    };
    let (_, auto_report) = solve(&auto, &matrix, &blocks, &rhs);
    assert_eq!(auto_report.route, LinearRoute::FgmresCpr);
}

/// Newton on every route, from one state with wells: the direct routes take the same iterations
/// and accept the same state to roundoff; CPR converges to the same state, and every iteration's
/// linear report is recorded.
#[test]
fn comp_linear_newton_converges_on_every_route() {
    let grid = BoxGrid::new(pinned::spe5().unwrap(), &SPE5_Z, 3, 2, 2);
    let rock = grid.rock();
    let relperm = relperm();
    let problem = NewtonProblem {
        spec: &grid.spec,
        layout: &grid.layout,
        rock,
        relperm: &relperm,
        faces: &grid.faces,
        wells: &grid.wells,
        previous_moles: &grid.previous,
        sources: &grid.sources,
        dt_days: 1.0,
    };
    let run = |r: LinearRoute| {
        let options = NewtonOptions {
            linear: route(r),
            ..NewtonOptions::default()
        };
        let (outcome, report) = solve_newton(&problem, &grid.state, options);
        (outcome.expect("Newton must converge"), report)
    };

    let (dense, dense_report) = run(LinearRoute::DenseLu);
    let (sparse, sparse_report) = run(LinearRoute::SparseLu);
    let (cpr, cpr_report) = run(LinearRoute::FgmresCpr);

    assert!(dense_report.iterations > 1, "the case must take a step");
    assert_eq!(sparse_report.iterations, dense_report.iterations);
    for report in [&dense_report, &sparse_report, &cpr_report] {
        assert_eq!(report.linear_history.len(), report.iterations - 1);
    }
    assert!(dense_report.min_pivot.is_finite());
    assert!(sparse_report.min_pivot.is_infinite());
    assert!(
        cpr_report.iterations <= dense_report.iterations + 1,
        "CPR {} vs dense {}",
        cpr_report.iterations,
        dense_report.iterations
    );

    for (route, accepted, tolerance) in [("sparse", &sparse, 1e-12), ("cpr", &cpr, 1e-9)] {
        for (a, b) in accepted.cells().iter().zip(dense.cells()) {
            assert!(
                (a.pressure_bar - b.pressure_bar).abs() <= tolerance * b.pressure_bar,
                "{route}: {} vs {}",
                a.pressure_bar,
                b.pressure_bar
            );
            for (za, zb) in a.independent_z().iter().zip(b.independent_z()) {
                assert!((za - zb).abs() <= tolerance, "{route}: z {za} vs {zb}");
            }
        }
    }
}

/// A linear failure inside Newton is the typed linear failure, so the timestep lifecycle can
/// classify it as linear rather than nonlinear.
#[test]
fn comp_linear_failure_surfaces_as_a_linear_newton_error() {
    let grid = BoxGrid::new(pinned::spe5().unwrap(), &SPE5_Z, 2, 2, 1);
    let rock = grid.rock();
    let relperm = relperm();
    let problem = NewtonProblem {
        spec: &grid.spec,
        layout: &grid.layout,
        rock,
        relperm: &relperm,
        faces: &grid.faces,
        wells: &grid.wells,
        previous_moles: &grid.previous,
        sources: &grid.sources,
        dt_days: 1.0,
    };
    let options = NewtonOptions {
        linear: LinearOptions {
            route: LinearRoute::FgmresCpr,
            relative_tolerance: 1e-30,
            restart: 1,
            max_iterations: 1,
        },
        ..NewtonOptions::default()
    };
    let (outcome, report) = solve_newton(&problem, &grid.state, options);
    assert!(
        matches!(
            outcome,
            Err(NewtonError::Linear(LinearFailure::NotConverged { .. }))
        ),
        "{outcome:?}"
    );
    assert_eq!(report.linear_history.len(), 1);
}

/// SPE3's row count and block size on the ten-component timing fluid: sparse LU and CPR agree.
#[test]
fn comp_linear_spe3_size_sparse_and_cpr_agree() {
    let grid = BoxGrid::new(ten_component_fluid(), &TEN_Z, 9, 9, 4);
    let (matrix, blocks, rhs) = grid.scaled_system();
    assert_eq!(matrix.rows(), 3240);

    let (sparse, sparse_report) = solve(&route(LinearRoute::SparseLu), &matrix, &blocks, &rhs);
    let options = route(LinearRoute::FgmresCpr);
    let (cpr, cpr_report) = solve(&options, &matrix, &blocks, &rhs);
    let sparse = sparse.unwrap();
    let cpr = cpr.unwrap();
    assert!(sparse_report.reduction() <= 1e-10, "{sparse_report:?}");
    assert!(cpr_report.reduction() <= options.relative_tolerance);
    let gap = max_relative_difference(&cpr, &sparse);
    assert!(gap <= 1e-7, "cpr vs sparse: {gap:e}");
}

fn wasm_heap_bytes() -> Option<usize> {
    #[cfg(target_arch = "wasm32")]
    {
        Some(core::arch::wasm32::memory_size(0) * 65536)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

fn measure(label: &str, grid: &BoxGrid, include_dense: bool) {
    let start = Instant::now();
    let assembled = grid.assemble();
    let assembly_ms = start.elapsed().as_secs_f64() * 1e3;
    let jacobian_bytes = assembled.jacobian.storage_bytes();
    let rows = assembled.jacobian.rows();
    drop(assembled);

    let (matrix, blocks, rhs) = grid.scaled_system();
    let mut routes = vec![LinearRoute::SparseLu, LinearRoute::FgmresCpr];
    if include_dense {
        routes.push(LinearRoute::DenseLu);
    }
    // One route per process, for a heap high-water mark that is that route's own.
    if let Ok(only) = std::env::var("RESSIM_COMP_LINEAR_COST_ROUTE") {
        routes.retain(|r| r.label() == only);
    }
    println!(
        "{label}: rows {rows}, block-sparse Jacobian {:.2} MB (dense would be {:.1} MB), \
         assembly {assembly_ms:.0} ms",
        jacobian_bytes as f64 / 1e6,
        (rows * rows * 8) as f64 / 1e6,
    );
    for r in routes {
        let before = wasm_heap_bytes();
        let start = Instant::now();
        let (outcome, report) = solve(&route(r), &matrix, &blocks, &rhs);
        let ms = start.elapsed().as_secs_f64() * 1e3;
        assert!(outcome.is_ok(), "{label} {}: {report:?}", r.label());
        let heap = match (before, wasm_heap_bytes()) {
            (Some(b), Some(a)) => format!(
                ", wasm heap {:.1} -> {:.1} MB",
                b as f64 / 1e6,
                a as f64 / 1e6
            ),
            _ => String::new(),
        };
        println!(
            "  {:<10} {ms:>9.1} ms  iterations {:>3}  reduction {:.1e}{heap}",
            r.label(),
            report.iterations,
            report.reduction()
        );
    }
}

/// Time and memory of one linear solve per route at SPE5 and SPE3 size. A measurement, not a
/// gate: run it `--release`, natively and under `wasm32-wasip1` (see
/// `docs/COMPOSITIONAL_VALIDATION.md`, the C10 linear-route record). On wasm the heap figure is
/// linear memory, which never shrinks, so it is the high-water mark up to that point; set
/// `RESSIM_COMP_LINEAR_COST_ROUTE` to one route's label to measure that route alone.
#[test]
#[ignore]
fn comp_linear_cost_spe5_and_spe3_size() {
    measure(
        "SPE5 size (7x7x3, N=6)",
        &BoxGrid::new(pinned::spe5().unwrap(), &SPE5_Z, 7, 7, 3),
        true,
    );
    measure(
        "SPE3 size (9x9x4, N=10 timing fluid)",
        &BoxGrid::new(ten_component_fluid(), &TEN_Z, 9, 9, 4),
        false,
    );
}
