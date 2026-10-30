//! V1b immiscible-water contract tests (`comp_water_*`, #46).
//!
//! What these establish, and against what:
//!
//! - **Layout and state**: the water column and row are reached by name, pressure stays in column
//!   0, and a hydrocarbon-free cell is rejected rather than represented.
//! - **Reduction to V1**: at `Sw = 0` with tables that reproduce the `Linear` curves, the
//!   hydrocarbon rows of the water system equal the water-free system's. Without water the engine
//!   does not run any of this code; the existing `comp_*` gates are the evidence for that path.
//! - **Jacobian**: the assembled Jacobian, wells and gravity included, against an independent
//!   central-difference one built by re-running the whole assembler.
//! - **Conservation**: a closed grid keeps every component, water included, to roundoff; with
//!   wells, the inventory change equals the accepted well cumulatives.
//! - **Stone II**: the compositional curves are the black-oil engine's, value for value.

use super::accumulation::{cell_accumulation_with_water, cell_inventory_with_water};
use super::api::{
    CASE_SCHEMA, CaseConfig, CompletionConfig, CompositionalCase, FluidChoice, GridConfig,
    RelPermConfig, WaterConfig, WellConfig, WellControlConfig,
};
use super::assembly::{AssemblyError, Face, assemble, numerical_jacobian};
use super::flux::Gravity;
use super::layout::{CellEquation, CellPrimary, CompositionalLayout, LayoutError};
use super::relperm::RelativePermeabilityModel;
use super::state::{CompositionalCellState, CompositionalState, RockView, StateError};
use super::timestep::{CompositionalRun, TimestepOptions};
use super::water::{StoneTwoRelPerm, WaterError, WaterModel, WaterProperties};
use super::wells::{
    CompositionalWell, InjectedFluid, SurfacePhase, WellControl, well_source_at_bhp,
};
use crate::fluid::pinned;
use crate::fluid::specification::FluidSpecification;
use crate::relperm::{RockFluidPropsThreePhase, SgofRow, SwofRow, ThreePhaseScalTables};

const GEOM_T: f64 = 8.526_988_8e-3 * 100.0 * (100.0 * 10.0) / 100.0;

fn swof(rows: &[(f64, f64, f64)]) -> Vec<SwofRow> {
    rows.iter()
        .map(|&(sw, krw, krow)| SwofRow {
            sw,
            krw,
            krow,
            pcow: None,
        })
        .collect()
}

fn sgof(rows: &[(f64, f64, f64)]) -> Vec<SgofRow> {
    rows.iter()
        .map(|&(sg, krg, krog)| SgofRow {
            sg,
            krg,
            krog,
            pcog: None,
        })
        .collect()
}

/// Tables that reproduce `kr_L = S_L`, `kr_V = S_V` at zero water: Stone II then collapses to
/// `krog(Sg)` exactly, because `krw(0) = 0` and `krow(0) = kro_max = 1`.
fn linear_equivalent_tables() -> StoneTwoRelPerm {
    StoneTwoRelPerm::new(
        swof(&[(0.0, 0.0, 1.0), (1.0, 1.0, 0.0)]),
        sgof(&[(0.0, 0.0, 1.0), (1.0, 1.0, 0.0)]),
    )
    .unwrap()
}

/// Curved, with connate water, residual oil and a critical gas saturation. Nodes every 0.1 so a
/// finite-difference state can sit mid-segment.
fn realistic_tables() -> StoneTwoRelPerm {
    StoneTwoRelPerm::new(
        swof(&[
            (0.15, 0.0, 0.9),
            (0.25, 0.01, 0.62),
            (0.35, 0.04, 0.40),
            (0.45, 0.09, 0.23),
            (0.55, 0.16, 0.11),
            (0.65, 0.25, 0.04),
            (0.75, 0.36, 0.0),
            (1.0, 0.6, 0.0),
        ]),
        sgof(&[
            (0.0, 0.0, 0.9),
            (0.05, 0.0, 0.80),
            (0.15, 0.03, 0.55),
            (0.25, 0.09, 0.35),
            (0.35, 0.18, 0.20),
            (0.45, 0.30, 0.09),
            (0.55, 0.45, 0.03),
            (0.85, 0.8, 0.0),
        ]),
    )
    .unwrap()
}

fn water_properties() -> WaterProperties {
    // A PVTW record of ordinary brine magnitude; the viscosibility is nonzero so its derivative is
    // exercised.
    WaterProperties::new(200.0, 1.02, 4.5e-5, 0.5, 1.0e-4, 1020.0).unwrap()
}

fn water_relperm(tables: StoneTwoRelPerm) -> RelativePermeabilityModel {
    RelativePermeabilityModel::StoneTwoWater(Box::new(WaterModel {
        properties: water_properties(),
        relperm: tables,
    }))
}

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

/// A column with per-cell pressure and water saturation, a common hydrocarbon composition.
fn column(
    spec: FluidSpecification,
    pressures: &[f64],
    water_saturations: &[f64],
    z: &[f64],
    gravity: bool,
) -> Grid {
    let cells = pressures.len();
    let layout = CompositionalLayout::new(spec.component_count(), cells, 0, 0)
        .unwrap()
        .with_water();
    let state = CompositionalState::new(
        &layout,
        pressures
            .iter()
            .zip(water_saturations)
            .map(|(&p, &sw)| CompositionalCellState::with_water(p, z.to_vec(), sw).unwrap())
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
                // Cell i+1 is 30 m deeper, so water and the hydrocarbons see different heads.
                gravity: if gravity {
                    Gravity::between(1000.0 + 30.0 * i as f64, 1030.0 + 30.0 * i as f64)
                } else {
                    Gravity::OFF
                },
            })
            .collect(),
        state,
    }
}

fn inventory(grid: &Grid, relperm: &RelativePermeabilityModel) -> Vec<Vec<f64>> {
    let rock = grid.rock();
    let water = relperm.water().map(|w| &w.properties);
    (0..grid.layout.cell_count())
        .map(|c| {
            cell_inventory_with_water(&grid.spec, &rock, water, c, grid.state.cell(c))
                .unwrap()
                .0
                .component_moles
        })
        .collect()
}

fn totals(per_cell: &[Vec<f64>]) -> Vec<f64> {
    let mut out = vec![0.0; per_cell[0].len()];
    for row in per_cell {
        for (t, v) in out.iter_mut().zip(row) {
            *t += v;
        }
    }
    out
}

// --- Layout and state -------------------------------------------------------------------------

#[test]
fn comp_water_layout_appends_one_named_column_and_row_and_keeps_pressure_first() {
    let dry = CompositionalLayout::new(3, 4, 0, 0).unwrap();
    let wet = dry.with_water();
    assert_eq!(dry.cell_block_size(), 3);
    assert_eq!(wet.cell_block_size(), 4);
    assert_eq!(wet.hydrocarbon_equation_count(), 3);

    // Every hydrocarbon offset is the water-free layout's.
    for cell in 0..4 {
        assert_eq!(
            wet.local_column(CellPrimary::Pressure).unwrap(),
            dry.local_column(CellPrimary::Pressure).unwrap()
        );
        for k in 0..2 {
            assert_eq!(
                wet.local_column(CellPrimary::Composition(k)).unwrap(),
                dry.local_column(CellPrimary::Composition(k)).unwrap()
            );
        }
        let column = wet
            .cell_unknown(cell, CellPrimary::WaterSaturation)
            .unwrap();
        assert_eq!(
            wet.split_cell_unknown(column),
            Some((cell, CellPrimary::WaterSaturation))
        );
        let row = wet.cell_equation(cell, CellEquation::WaterBalance).unwrap();
        assert_eq!(
            wet.split_cell_equation(row),
            Some((cell, CellEquation::WaterBalance))
        );
        assert!(wet.is_cell_pressure_column(cell * 4));
        assert!(!wet.is_cell_pressure_column(column));
    }
    assert_eq!(wet.to_linear_block_layout().cell_block_size, 4);

    assert_eq!(
        dry.local_column(CellPrimary::WaterSaturation),
        Err(LayoutError::NoWaterPhase)
    );
    assert_eq!(
        dry.local_row(CellEquation::WaterBalance),
        Err(LayoutError::NoWaterPhase)
    );
}

#[test]
fn comp_water_state_rejects_a_hydrocarbon_free_cell_and_water_in_a_dry_layout() {
    assert!(matches!(
        CompositionalCellState::with_water(200.0, vec![0.3, 0.4], 1.0),
        Err(StateError::WaterSaturationOutOfRange { .. })
    ));
    assert!(matches!(
        CompositionalCellState::with_water(200.0, vec![0.3, 0.4], -1e-3),
        Err(StateError::WaterSaturationOutOfRange { .. })
    ));
    let dry = CompositionalLayout::new(3, 1, 0, 0).unwrap();
    let cell = CompositionalCellState::with_water(200.0, vec![0.3, 0.4], 0.2).unwrap();
    assert!(matches!(
        CompositionalState::new(&dry, vec![cell.clone()]),
        Err(StateError::WaterWithoutWaterLayout { .. })
    ));
    assert!(CompositionalState::new(&dry.with_water(), vec![cell]).is_ok());
}

#[test]
fn comp_water_assembly_refuses_a_layout_that_disagrees_with_the_relperm() {
    let grid = column(
        pinned::ternary().unwrap(),
        &[170.0, 150.0],
        &[0.3, 0.3],
        &[0.2, 0.5],
        false,
    );
    let rock = grid.rock();
    let previous = vec![vec![0.0; 4]; 2];
    let result = assemble(
        &grid.spec,
        &grid.layout,
        &rock,
        &RelativePermeabilityModel::Linear,
        &grid.state,
        &grid.faces,
        &[],
        &previous,
        &previous,
        1.0,
    );
    assert!(matches!(result, Err(AssemblyError::WaterMismatch { .. })));
}

// --- Physics ----------------------------------------------------------------------------------

#[test]
fn comp_water_properties_follow_pvtw() {
    let w = water_properties();
    // At the reference pressure, b_w = 1/B_w and mu = mu_ref exactly.
    assert_eq!(w.reciprocal_fvf(200.0), 1.0 / 1.02);
    assert_eq!(w.viscosity_cp(200.0), 0.5);
    // Above it, the second-order expansion of exp(c dp).
    let x: f64 = 4.5e-5 * 100.0;
    let expected = (1.0 + x + 0.5 * x * x) / 1.02;
    assert!((w.reciprocal_fvf(300.0) - expected).abs() < 1e-15);
    assert!(w.molar_density(300.0) > w.molar_density(200.0));
    // Surface volume inverts the molar conversion at the surface density.
    let moles = 1.0e5;
    let v = w.surface_volume_m3(moles);
    assert!((v * 1020.0 / super::water::WATER_MOLAR_MASS_KG_PER_MOL - moles).abs() < 1e-6);
}

#[test]
fn comp_water_stone_two_is_the_black_oil_engines() {
    let tables = realistic_tables();
    let black_oil = RockFluidPropsThreePhase {
        s_wc: 0.15,
        s_or: 0.0,
        n_w: 1.0,
        n_o: 1.0,
        k_rw_max: 0.6,
        k_ro_max: 0.9,
        s_gc: 0.0,
        s_gr: 0.0,
        s_org: 0.0,
        n_g: 1.0,
        k_rg_max: 0.8,
        tables: Some(ThreePhaseScalTables {
            swof: swof(&[
                (0.15, 0.0, 0.9),
                (0.25, 0.01, 0.62),
                (0.35, 0.04, 0.40),
                (0.45, 0.09, 0.23),
                (0.55, 0.16, 0.11),
                (0.65, 0.25, 0.04),
                (0.75, 0.36, 0.0),
                (1.0, 0.6, 0.0),
            ]),
            sgof: sgof(&[
                (0.0, 0.0, 0.9),
                (0.05, 0.0, 0.80),
                (0.15, 0.03, 0.55),
                (0.25, 0.09, 0.35),
                (0.35, 0.18, 0.20),
                (0.45, 0.30, 0.09),
                (0.55, 0.45, 0.03),
                (0.85, 0.8, 0.0),
            ]),
        }),
    };
    for &sw in &[0.15, 0.2, 0.33, 0.5, 0.71] {
        assert_eq!(tables.kr_water(sw), black_oil.k_rw(sw));
        for &sg in &[0.0, 0.03, 0.12, 0.3, 0.5] {
            if sw + sg >= 1.0 {
                continue;
            }
            assert_eq!(tables.kr_vapour(sg), black_oil.k_rg(sg));
            assert_eq!(
                tables.kr_liquid(sw, sg),
                black_oil.k_ro_stone2(sw, sg),
                "Stone II at Sw = {sw}, Sg = {sg}"
            );
        }
    }
}

#[test]
fn comp_water_tables_reject_capillary_pressure_and_inconsistent_oil_endpoints() {
    let mut with_pc = swof(&[(0.2, 0.0, 1.0), (1.0, 1.0, 0.0)]);
    with_pc[0].pcow = Some(0.3);
    assert!(matches!(
        StoneTwoRelPerm::new(with_pc, sgof(&[(0.0, 0.0, 1.0), (0.8, 1.0, 0.0)])),
        Err(WaterError::CapillaryPressureNotSupported { .. })
    ));
    // Explicit zero capillary pressure is the model, not a rejection.
    let mut zero_pc = swof(&[(0.2, 0.0, 1.0), (1.0, 1.0, 0.0)]);
    zero_pc[0].pcow = Some(0.0);
    assert!(StoneTwoRelPerm::new(zero_pc, sgof(&[(0.0, 0.0, 1.0), (0.8, 1.0, 0.0)])).is_ok());
    assert!(matches!(
        StoneTwoRelPerm::new(
            swof(&[(0.2, 0.0, 1.0), (1.0, 1.0, 0.0)]),
            sgof(&[(0.0, 0.0, 0.8), (0.8, 1.0, 0.0)])
        ),
        Err(WaterError::Table(_))
    ));
}

/// The hydrocarbon rows of the water system at `Sw = 0`, on Stone II tables that reproduce the
/// `Linear` curves there, are the water-free system's rows. The water row and the `Sw` column are
/// what is added; nothing about the hydrocarbon equations changes.
#[test]
fn comp_water_zero_water_reduces_to_the_water_free_system() {
    let spec = pinned::ternary().unwrap();
    let pressures = [175.0, 160.0, 150.0];
    let z = [0.2, 0.5];
    let wet = column(spec.clone(), &pressures, &[0.0; 3], &z, false);
    let dry_layout = CompositionalLayout::new(3, 3, 0, 0).unwrap();
    let dry_state = CompositionalState::new(
        &dry_layout,
        pressures
            .iter()
            .map(|&p| CompositionalCellState::new(p, z.to_vec()).unwrap())
            .collect(),
    )
    .unwrap();
    let rock = wet.rock();
    let wet_relperm = water_relperm(linear_equivalent_tables());
    let dry_relperm = RelativePermeabilityModel::Linear;

    // A previous inventory that is not stationary, so the residual has something in it.
    let wet_previous: Vec<Vec<f64>> = inventory(&wet, &wet_relperm)
        .into_iter()
        .map(|row| row.iter().map(|v| v * 0.999).collect())
        .collect();
    let dry_previous: Vec<Vec<f64>> = wet_previous.iter().map(|r| r[..3].to_vec()).collect();

    let a_wet = assemble(
        &spec,
        &wet.layout,
        &rock,
        &wet_relperm,
        &wet.state,
        &wet.faces,
        &[],
        &wet_previous,
        &vec![vec![0.0; 4]; 3],
        0.5,
    )
    .unwrap();
    let a_dry = assemble(
        &spec,
        &dry_layout,
        &rock,
        &dry_relperm,
        &dry_state,
        &wet.faces,
        &[],
        &dry_previous,
        &vec![vec![0.0; 3]; 3],
        0.5,
    )
    .unwrap();

    let mut worst = 0.0f64;
    for cell in 0..3 {
        for i in 0..3 {
            let wet_row = wet
                .layout
                .cell_equation(cell, CellEquation::ComponentBalance(i))
                .unwrap();
            let dry_row = dry_layout
                .cell_equation(cell, CellEquation::ComponentBalance(i))
                .unwrap();
            let scale = a_dry.scaling[cell].row_scale[i];
            worst = worst.max((a_wet.residual[wet_row] - a_dry.residual[dry_row]).abs() / scale);
            for other in 0..3 {
                for primary in [
                    CellPrimary::Pressure,
                    CellPrimary::Composition(0),
                    CellPrimary::Composition(1),
                ] {
                    let wc = wet.layout.cell_unknown(other, primary).unwrap();
                    let dc = dry_layout.cell_unknown(other, primary).unwrap();
                    let d = a_dry.jacobian.get(dry_row, dc);
                    let w = a_wet.jacobian.get(wet_row, wc);
                    worst = worst.max((w - d).abs() / scale);
                }
            }
        }
        // No water anywhere: the water row is exactly zero.
        let water_row = wet
            .layout
            .cell_equation(cell, CellEquation::WaterBalance)
            .unwrap();
        assert_eq!(a_wet.residual[water_row], 0.0);
    }
    assert!(
        worst < 1e-12,
        "hydrocarbon rows differ from the water-free system by {worst:e}"
    );
}

/// Three-phase closure, from the inventory's own saturations: `Sw + S_L + S_V = 1` and the
/// hydrocarbon moles occupy exactly `(1 - Sw)` of the pore volume.
#[test]
fn comp_water_saturations_close_and_hydrocarbons_fill_their_share() {
    let grid = column(
        pinned::ternary().unwrap(),
        &[170.0, 160.0, 150.0],
        &[0.1, 0.4, 0.8],
        &[0.2, 0.5],
        false,
    );
    let rock = grid.rock();
    let props = water_properties();
    for cell in 0..3 {
        let c = grid.state.cell(cell);
        let (wet, flash) =
            cell_inventory_with_water(&grid.spec, &rock, Some(&props), cell, c).unwrap();
        let sw = c.water_saturation;
        let s_v = wet.vapour_saturation;
        let s_l = (1.0 - sw) * (1.0 - flash.vapour_saturation());
        assert!((sw + s_l + s_v - 1.0).abs() < 1e-14);
        // The hydrocarbon volume, from the moles and the mixture molar volume, is (1 - Sw) PV.
        let hc_volume = wet.total_moles * wet.mixture_molar_volume;
        assert!((hc_volume / wet.pore_volume_m3 - (1.0 - sw)).abs() < 1e-12);
        let water_volume = wet.component_moles[3] / props.molar_density(c.pressure_bar);
        assert!((water_volume / wet.pore_volume_m3 - sw).abs() < 1e-12);
    }
}

/// The accumulation's `Sw` column and water row, against central differences on the cell alone.
#[test]
fn comp_water_accumulation_jacobian_matches_finite_differences() {
    let spec = pinned::ternary().unwrap();
    let props = water_properties();
    let pv = [1000.0];
    let rock = RockView {
        pore_volume_ref_m3: &pv,
        reference_pressure_bar: 200.0,
        compressibility_per_bar: 4.0e-5,
    };
    for &sw in &[0.05, 0.37, 0.93] {
        let cell = CompositionalCellState::with_water(165.0, vec![0.2, 0.5], sw).unwrap();
        let zero = vec![0.0; 4];
        let acc =
            cell_accumulation_with_water(&spec, &rock, Some(&props), 0, &cell, &zero, &zero, 1.0)
                .unwrap();
        let moles = |c: &CompositionalCellState| {
            cell_inventory_with_water(&spec, &rock, Some(&props), 0, c)
                .unwrap()
                .0
                .component_moles
        };
        let steps = [1e-3, 1e-6, 1e-6, 1e-7];
        for (v, step) in steps.iter().enumerate() {
            let perturbed = |sign: f64| {
                let mut c = cell.clone();
                match v {
                    0 => c.pressure_bar += sign * step,
                    3 => c.water_saturation += sign * step,
                    k => c.independent_z_mut()[k - 1] += sign * step,
                }
                moles(&c)
            };
            let (up, down) = (perturbed(1.0), perturbed(-1.0));
            for i in 0..4 {
                let numerical = (up[i] - down[i]) / (2.0 * step);
                let analytic = acc.jacobian[i][v];
                let scale = numerical
                    .abs()
                    .max(1e-3 * acc.inventory.component_moles[i].abs());
                assert!(
                    (analytic - numerical).abs() <= 1e-5 * scale.max(1e-12),
                    "Sw = {sw}: d n_{i} / d u_{v}: analytic {analytic:e}, numerical {numerical:e}"
                );
            }
        }
    }
}

/// The whole assembled Jacobian — accumulation, face flux with gravity, a water injector and a
/// producer — against a numerical one built by re-running the assembler. The FD acceptance gate
/// the issue names.
#[test]
fn comp_water_assembled_jacobian_matches_a_numerical_jacobian_entrywise() {
    let relperm = water_relperm(realistic_tables());
    let cases = [
        (
            "ternary two-phase, gravity",
            pinned::ternary().unwrap(),
            vec![172.0, 163.0, 151.0],
            vec![0.42, 0.31, 0.28],
            vec![0.2, 0.5],
            true,
        ),
        (
            "binary, flat",
            pinned::binary().unwrap(),
            vec![205.0, 190.0, 182.0],
            vec![0.58, 0.33, 0.21],
            vec![0.6],
            false,
        ),
    ];
    let mut worst = (0.0f64, String::new());
    for (name, spec, pressures, sws, z, gravity) in cases {
        let grid = column(spec, &pressures, &sws, &z, gravity);
        let rock = grid.rock();
        let b = grid.layout.cell_block_size();
        let previous: Vec<Vec<f64>> = inventory(&grid, &relperm)
            .into_iter()
            .map(|row| row.iter().map(|v| v * 0.998).collect())
            .collect();
        let sources = vec![vec![0.0; b]; 3];
        let mut injector = CompositionalWell::single(
            "WINJ",
            0,
            2.0,
            WellControl::Bhp {
                target_bar: pressures[0] + 25.0,
            },
        );
        injector.injection = Some(InjectedFluid::Water);
        let producer = CompositionalWell::single(
            "PROD",
            2,
            2.0,
            WellControl::Bhp {
                target_bar: pressures[2] - 20.0,
            },
        );
        let wells = [injector, producer];

        let analytic = assemble(
            &grid.spec,
            &grid.layout,
            &rock,
            &relperm,
            &grid.state,
            &grid.faces,
            &wells,
            &previous,
            &sources,
            0.7,
        )
        .unwrap();
        let numerical = numerical_jacobian(
            &grid.spec,
            &grid.layout,
            &rock,
            &relperm,
            &grid.state,
            &grid.faces,
            &wells,
            &previous,
            &sources,
            0.7,
            1e-3,
            1e-7,
        )
        .unwrap();

        let size = 3 * b;
        for column_index in 0..size {
            let scale = (0..size)
                .map(|r| numerical[r][column_index].abs())
                .fold(0.0f64, f64::max)
                .max(1e-8);
            for row in 0..size {
                let e = (analytic.jacobian.get(row, column_index) - numerical[row][column_index])
                    .abs()
                    / scale;
                if e > worst.0 {
                    let what = grid.layout.split_cell_unknown(column_index);
                    worst = (e, format!("{name}[{row}][{column_index}] {what:?}"));
                }
            }
        }
    }
    assert!(
        worst.0 < 1e-5,
        "the assembled water Jacobian disagrees with the numerical one: worst scaled error {:e} \
         at {}",
        worst.0,
        worst.1
    );
}

/// C10's linear gate on the enlarged layout: Newton on a water system converges on every linear
/// route, and the routes agree. CPR's true-IMPES weights solve against the `N+1` accumulation
/// block, water row included.
#[test]
fn comp_water_newton_converges_on_every_linear_route() {
    use super::linear::{LinearOptions, LinearRoute};
    use super::newton::{NewtonOptions, NewtonProblem, solve_newton};
    let relperm = water_relperm(realistic_tables());
    let grid = column(
        pinned::ternary().unwrap(),
        &[170.0, 166.0, 162.0, 158.0, 154.0],
        &[0.25, 0.3, 0.35, 0.4, 0.45],
        &[0.2, 0.5],
        true,
    );
    let rock = grid.rock();
    let previous = inventory(&grid, &relperm);
    let sources = vec![vec![0.0; 4]; 5];
    let mut injector =
        CompositionalWell::single("WINJ", 0, 2.0, WellControl::Bhp { target_bar: 200.0 });
    injector.injection = Some(InjectedFluid::Water);
    let producer =
        CompositionalWell::single("PROD", 4, 2.0, WellControl::Bhp { target_bar: 140.0 });
    let wells = [injector, producer];
    let problem = NewtonProblem {
        spec: &grid.spec,
        layout: &grid.layout,
        rock,
        relperm: &relperm,
        faces: &grid.faces,
        wells: &wells,
        previous_moles: &previous,
        sources: &sources,
        dt_days: 1.0,
    };
    let run = |route: LinearRoute| {
        let options = NewtonOptions {
            linear: LinearOptions {
                route,
                ..LinearOptions::default()
            },
            ..NewtonOptions::default()
        };
        let (outcome, report) = solve_newton(&problem, &grid.state, options);
        (outcome.expect("Newton must converge"), report)
    };
    let (dense, dense_report) = run(LinearRoute::DenseLu);
    let (sparse, _) = run(LinearRoute::SparseLu);
    let (cpr, cpr_report) = run(LinearRoute::FgmresCpr);
    assert!(dense_report.iterations > 1, "the case must take a step");
    assert!(cpr_report.iterations <= dense_report.iterations + 1);
    for (route, accepted, tolerance) in [("sparse", &sparse, 1e-12), ("cpr", &cpr, 1e-9)] {
        for (a, b) in accepted.cells().iter().zip(dense.cells()) {
            assert!(
                (a.pressure_bar - b.pressure_bar).abs() <= tolerance * b.pressure_bar,
                "{route}: p {} vs {}",
                a.pressure_bar,
                b.pressure_bar
            );
            assert!(
                (a.water_saturation - b.water_saturation).abs() <= tolerance,
                "{route}: Sw {} vs {}",
                a.water_saturation,
                b.water_saturation
            );
            for (za, zb) in a.independent_z().iter().zip(b.independent_z()) {
                assert!((za - zb).abs() <= tolerance, "{route}: z {za} vs {zb}");
            }
        }
    }
}

// --- Conservation and displacement -------------------------------------------------------------

struct Run {
    grid: Grid,
    run: CompositionalRun,
}

fn run_on(grid: Grid) -> Run {
    let run = CompositionalRun::new(grid.state.clone());
    Run { grid, run }
}

impl Run {
    fn step(
        &mut self,
        relperm: &RelativePermeabilityModel,
        wells: &[CompositionalWell],
        dt: f64,
    ) -> bool {
        let rock = RockView {
            pore_volume_ref_m3: &self.grid.pore_volumes,
            reference_pressure_bar: 200.0,
            compressibility_per_bar: 4.0e-5,
        };
        let b = self.grid.layout.cell_block_size();
        let sources = vec![vec![0.0; b]; self.grid.layout.cell_count()];
        let report = self.run.step(
            &self.grid.spec,
            &self.grid.layout,
            &rock,
            relperm,
            &self.grid.faces,
            wells,
            &sources,
            dt,
            TimestepOptions::default(),
        );
        if let Some((kind, message)) = &report.failure
            && !report.succeeded()
        {
            panic!("step failed: {kind:?}: {message}");
        }
        report.succeeded()
    }

    fn inventory(&self, relperm: &RelativePermeabilityModel) -> Vec<f64> {
        let rock = RockView {
            pore_volume_ref_m3: &self.grid.pore_volumes,
            reference_pressure_bar: 200.0,
            compressibility_per_bar: 4.0e-5,
        };
        totals(
            &self
                .run
                .accepted_inventory_with_water(
                    &self.grid.spec,
                    &rock,
                    relperm.water().map(|w| &w.properties),
                )
                .unwrap(),
        )
    }
}

/// A closed column with gravity and an unstable water column (water above gas) redistributes and
/// keeps every component, water included, to roundoff.
#[test]
fn comp_water_closed_system_conserves_every_component() {
    let relperm = water_relperm(realistic_tables());
    let grid = column(
        pinned::ternary().unwrap(),
        &[175.0, 168.0, 160.0, 150.0],
        &[0.7, 0.5, 0.3, 0.2],
        &[0.2, 0.5],
        true,
    );
    let mut run = run_on(grid);
    let before = run.inventory(&relperm);
    for _ in 0..5 {
        assert!(run.step(&relperm, &[], 2.0));
    }
    let after = run.inventory(&relperm);
    for (i, (a, b)) in after.iter().zip(&before).enumerate() {
        assert!(
            ((a - b) / b).abs() < 1e-9,
            "component {i}: {b} -> {a} in a closed system"
        );
    }
    // And it moved: water fell, so the top cell lost water.
    assert!(run.run.state().cell(0).water_saturation < 0.7);
}

/// Water injected into hydrocarbon-filled cells, with a producer at the far end: water saturation
/// rises at the injector, and the inventory change equals the accepted well cumulatives.
#[test]
fn comp_water_injection_into_hydrocarbon_cells_balances_against_the_wells() {
    let relperm = water_relperm(realistic_tables());
    let grid = column(
        pinned::binary().unwrap(),
        &[200.0; 5],
        &[0.15; 5],
        &[0.6],
        false,
    );
    let mut injector =
        CompositionalWell::single("WINJ", 0, 0.3, WellControl::Bhp { target_bar: 260.0 });
    injector.injection = Some(InjectedFluid::Water);
    let producer =
        CompositionalWell::single("PROD", 4, 3.0, WellControl::Bhp { target_bar: 150.0 });
    let wells = [injector, producer];
    let mut run = run_on(grid);
    let before = run.inventory(&relperm);
    for _ in 0..10 {
        assert!(run.step(&relperm, &wells, 1.0));
    }
    let after = run.inventory(&relperm);
    let cumulative = run.run.cumulative_well_moles();
    for i in 0..3 {
        let wells_total: f64 = cumulative.iter().map(|w| w[i]).sum();
        let change = after[i] - before[i];
        assert!(
            (change - wells_total).abs() <= 1e-8 * before[i],
            "component {i}: inventory changed by {change}, wells moved {wells_total}"
        );
    }
    // Water entered through the injector only, and hydrocarbons left through the producer only.
    assert!(cumulative[0][2] > 0.0);
    assert!(cumulative[0][0] == 0.0 && cumulative[0][1] == 0.0);
    assert!(cumulative[1][0] < 0.0 && cumulative[1][1] < 0.0);
    assert!(run.run.state().cell(0).water_saturation > 0.3);
}

/// A water injector on a surface-rate target. This is the water-side half of #48: the rate row's
/// gradient has one entry for water in the `N+1` block, and the BHP unknown is eliminated exactly
/// as it is for a hydrocarbon well.
#[test]
fn comp_water_rate_controlled_injector_holds_its_surface_target() {
    let relperm = water_relperm(realistic_tables());
    let grid = column(
        pinned::binary().unwrap(),
        &[200.0; 5],
        &[0.15; 5],
        &[0.6],
        false,
    );
    let mut bhp_well =
        CompositionalWell::single("WINJ", 0, 0.3, WellControl::Bhp { target_bar: 260.0 });
    bhp_well.injection = Some(InjectedFluid::Water);
    let at_bhp =
        well_source_at_bhp(&grid.spec, &relperm, &bhp_well, grid.state.cells(), 260.0).unwrap();
    let water_moles = at_bhp.total_component_moles_per_day(3)[2];
    assert!(water_moles > 0.0);
    let target_volume = water_properties().surface_volume_m3(water_moles);

    let mut rate_well = CompositionalWell::single(
        "WINJ",
        0,
        0.3,
        WellControl::SurfaceRate {
            target_m3_per_day: target_volume,
            phase: SurfacePhase::Water,
            bhp_limit_bar: 400.0,
        },
    );
    rate_well.injection = Some(InjectedFluid::Water);
    let producer =
        CompositionalWell::single("PROD", 4, 3.0, WellControl::Bhp { target_bar: 150.0 });
    let wells = [rate_well, producer];

    let mut run = run_on(grid);
    assert!(run.step(&relperm, &wells, 0.01));
    let cumulative = run.run.cumulative_well_moles();
    let injected_moles = cumulative[0][2];
    let achieved_volume =
        water_properties().surface_volume_m3(injected_moles) / run.run.time_days();
    assert!(
        (achieved_volume - target_volume).abs() / target_volume < 1e-7,
        "injected {achieved_volume} m³/day against a target of {target_volume}"
    );
    assert!(
        run.run.well_bhp_bar()[0] > 200.0,
        "the water injector should have solved a BHP above the cell pressure"
    );
}

/// Hydrocarbon injected into nearly water-filled cells: the small-hydrocarbon limit. The step bound
/// keeps `Sw` below one and the balance closes.
#[test]
fn comp_water_hydrocarbon_injection_into_water_filled_cells() {
    let relperm = water_relperm(realistic_tables());
    let grid = column(
        pinned::ternary().unwrap(),
        &[160.0; 4],
        &[0.93; 4],
        &[0.2, 0.5],
        false,
    );
    let mut injector =
        CompositionalWell::single("GINJ", 0, 3.0, WellControl::Bhp { target_bar: 200.0 });
    injector.injection = Some(InjectedFluid::Hydrocarbon(vec![0.9, 0.1, 0.0]));
    let producer =
        CompositionalWell::single("PROD", 3, 3.0, WellControl::Bhp { target_bar: 140.0 });
    let wells = [injector, producer];
    let mut run = run_on(grid);
    let before = run.inventory(&relperm);
    for _ in 0..8 {
        assert!(run.step(&relperm, &wells, 0.5));
    }
    let after = run.inventory(&relperm);
    let cumulative = run.run.cumulative_well_moles();
    for i in 0..4 {
        let wells_total: f64 = cumulative.iter().map(|w| w[i]).sum();
        // Newton closes each step's balance to 1e-8 of that step's previous inventory, and the
        // injected component grows several-fold here, so the bound is per step on the larger end.
        let bound = 8.0 * 1e-8 * before[i].max(after[i]);
        assert!(
            (after[i] - before[i] - wells_total).abs() <= bound,
            "component {i}: before {}, after {}, wells {wells_total}",
            before[i],
            after[i]
        );
    }
    assert!(run.run.state().cell(0).water_saturation < 0.93);
}

// --- Engine boundary --------------------------------------------------------------------------

fn water_case(initial_sw: f64) -> CaseConfig {
    CaseConfig {
        schema: CASE_SCHEMA.to_string(),
        fluid: FluidChoice::PinnedBinary,
        grid: GridConfig {
            cells: 5,
            dx_m: 20.0,
            dy_m: 10.0,
            dz_m: 10.0,
            porosity: 0.2,
            permeability_md: 100.0,
            rock_reference_pressure_bar: 200.0,
            rock_compressibility_per_bar: 0.0,
            box_grid: None,
        },
        relperm: RelPermConfig::StoneIi {
            swof: swof(&[(0.15, 0.0, 0.9), (0.6, 0.2, 0.1), (1.0, 0.6, 0.0)]),
            sgof: sgof(&[(0.0, 0.0, 0.9), (0.05, 0.0, 0.8), (0.85, 0.8, 0.0)]),
        },
        initial_pressure_bar: 200.0,
        initial_composition: vec![0.6, 0.4],
        wells: vec![
            WellConfig {
                id: "WINJ".to_string(),
                completions: vec![CompletionConfig {
                    cell: 0,
                    well_index: 2.0,
                    head_offset_bar: 0.0,
                }],
                control: WellControlConfig::Bhp { target_bar: 240.0 },
                injection_composition: None,
                inject_water: true,
            },
            WellConfig {
                id: "PROD".to_string(),
                completions: vec![CompletionConfig {
                    cell: 4,
                    well_index: 2.0,
                    head_offset_bar: 0.0,
                }],
                control: WellControlConfig::Bhp { target_bar: 170.0 },
                injection_composition: None,
                inject_water: false,
            },
        ],
        gravity_enabled: false,
        initial_pressure_gradient_bar_per_m: None,
        water: Some(WaterConfig {
            reference_pressure_bar: 200.0,
            formation_volume_factor: 1.02,
            compressibility_per_bar: 4.5e-5,
            viscosity_cp: 0.5,
            viscosibility_per_bar: 0.0,
            surface_density_kg_per_m3: 1020.0,
            initial_saturation: initial_sw,
        }),
    }
}

#[test]
fn comp_water_api_requires_water_and_stone_two_together() {
    assert!(water_case(0.15).validate().is_ok());

    let mut no_water = water_case(0.15);
    no_water.water = None;
    assert!(no_water.validate().is_err());

    let mut two_phase = water_case(0.15);
    two_phase.relperm = RelPermConfig::Linear;
    assert!(two_phase.validate().is_err());

    assert!(water_case(1.0).validate().is_err());

    let mut both = water_case(0.15);
    both.wells[0].injection_composition = Some(vec![1.0, 0.0]);
    assert!(both.validate().is_err());
}

#[test]
fn comp_water_api_runs_reports_water_and_round_trips_a_checkpoint() {
    let mut case = CompositionalCase::new(water_case(0.15)).unwrap();
    for _ in 0..4 {
        let outcome = case.step(1.0);
        assert!(outcome.succeeded(), "{:?}", outcome.failure);
    }
    let snapshot = case.snapshot();
    let water = snapshot.water.as_ref().expect("a water case reports water");
    assert_eq!(water.saturation.len(), 5);
    assert!(water.saturation[0] > 0.15);
    assert_eq!(snapshot.inventory.len(), 2);
    assert_eq!(snapshot.cumulative_well_moles[0].len(), 3);
    for (cell, &sw) in water.saturation.iter().enumerate() {
        assert!(sw + snapshot.vapour_saturation[cell] <= 1.0 + 1e-12);
    }

    let checkpoint = case.checkpoint();
    let json = serde_json::to_string(&checkpoint).unwrap();
    let restored = CompositionalCase::restore(&serde_json::from_str(&json).unwrap()).unwrap();
    assert_eq!(restored.snapshot().water, snapshot.water);
    assert_eq!(restored.snapshot().pressure, snapshot.pressure);
}

// --- Against JutulDarcy ------------------------------------------------------------------------
//
// `opm/compositional/water_injection/reference.json`, generated by
// `tools/opm_compositional/run-water-injection.sh`: water injected into SPE3 gas above its dew
// point, JutulDarcy 0.3.7 at four report steps. The oracle is shown to converge in time before it
// is used (COMPOSITIONAL_C12_FORENSICS.md), and ResSim is compared at two of those steps, so the
// comparison is between two simulators converging to the same limit rather than one number.

#[derive(serde::Deserialize)]
struct WaterReference {
    initial_pressure_bar: f64,
    initial_water_saturation: f64,
    cells: usize,
    dx_m: f64,
    dy_m: f64,
    dz_m: f64,
    porosity: f64,
    permeability_md: f64,
    injector_bhp_bar: f64,
    producer_bhp_bar: f64,
    well_index: f64,
    horizon_days: f64,
    pvtw: ReferencePvtw,
    swof: Vec<[f64; 3]>,
    sgof: Vec<[f64; 3]>,
    matched: ReferenceRun,
    finest: ReferenceRun,
    runs: Vec<ReferenceRunFacts>,
    convergence: Vec<ReferenceConvergence>,
}

#[derive(serde::Deserialize)]
struct ReferencePvtw {
    reference_pressure_bar: f64,
    formation_volume_factor: f64,
    compressibility_per_bar: f64,
    viscosity_cp: f64,
    viscosibility_per_bar: f64,
    surface_density_kg_per_m3: f64,
}

#[derive(serde::Deserialize)]
struct ReferenceRun {
    dt_days: f64,
    reports: Vec<ReferenceReport>,
}

#[derive(serde::Deserialize)]
struct ReferenceReport {
    day: f64,
    p: Vec<f64>,
    sw: Vec<f64>,
}

#[derive(serde::Deserialize)]
struct ReferenceRunFacts {
    single_vapour: bool,
    max_composition_drift: f64,
}

#[derive(serde::Deserialize)]
struct ReferenceConvergence {
    max_dp_bar: f64,
    max_dsw: f64,
}

fn water_reference() -> WaterReference {
    let raw = include_str!("../../../../../opm/compositional/water_injection/reference.json");
    serde_json::from_str(raw).expect("water-injection reference parses")
}

/// What a ResSim run did at each whole day: `(day, p, Sw)`, plus the worst composition drift and
/// whether every state stayed single-phase vapour.
struct ResSimTrajectory {
    days: Vec<(f64, Vec<f64>, Vec<f64>)>,
    max_composition_drift: f64,
    single_vapour: bool,
    water_balance_error: f64,
}

fn run_water_reference(r: &WaterReference, dt_days: f64, horizon_days: f64) -> ResSimTrajectory {
    use crate::fluid::flash::{PhaseState, flash};
    let spec = pinned::spe3().unwrap();
    let z0 = pinned::SPE3_COMPOSITION;
    let layout = CompositionalLayout::new(9, r.cells, 2, 2)
        .unwrap()
        .with_water();
    let cells = (0..r.cells)
        .map(|_| {
            CompositionalCellState::with_water(
                r.initial_pressure_bar,
                z0[..8].to_vec(),
                r.initial_water_saturation,
            )
            .unwrap()
        })
        .collect();
    let mut run = CompositionalRun::new(CompositionalState::new(&layout, cells).unwrap());
    let pore_volumes = vec![r.dx_m * r.dy_m * r.dz_m * r.porosity; r.cells];
    let rock = RockView {
        pore_volume_ref_m3: &pore_volumes,
        reference_pressure_bar: r.initial_pressure_bar,
        compressibility_per_bar: 0.0,
    };
    let geom_t = 8.526_988_8e-3 * r.permeability_md * (r.dy_m * r.dz_m) / r.dx_m;
    let faces: Vec<Face> = (0..r.cells - 1)
        .map(|i| Face {
            cell_i: i,
            cell_j: i + 1,
            geom_t,
            gravity: Gravity::OFF,
        })
        .collect();
    let table = |rows: &[[f64; 3]]| rows.iter().map(|r| (r[0], r[1], r[2])).collect::<Vec<_>>();
    let w = &r.pvtw;
    let relperm = RelativePermeabilityModel::StoneTwoWater(Box::new(WaterModel {
        properties: WaterProperties::new(
            w.reference_pressure_bar,
            w.formation_volume_factor,
            w.compressibility_per_bar,
            w.viscosity_cp,
            w.viscosibility_per_bar,
            w.surface_density_kg_per_m3,
        )
        .unwrap(),
        relperm: StoneTwoRelPerm::new(swof(&table(&r.swof)), sgof(&table(&r.sgof))).unwrap(),
    }));
    let mut injector = CompositionalWell::single(
        "Injector",
        0,
        r.well_index,
        WellControl::Bhp {
            target_bar: r.injector_bhp_bar,
        },
    );
    injector.injection = Some(InjectedFluid::Water);
    let producer = CompositionalWell::single(
        "Producer",
        r.cells - 1,
        r.well_index,
        WellControl::Bhp {
            target_bar: r.producer_bhp_bar,
        },
    );
    let wells = [injector, producer];
    let water = relperm.water().map(|w| &w.properties);
    let initial = totals(
        &run.accepted_inventory_with_water(&spec, &rock, water)
            .unwrap(),
    );
    let sources = vec![vec![0.0; 10]; r.cells];
    let mut options = TimestepOptions::default();
    options.newton.tolerance = 1e-10;

    let steps = (horizon_days / dt_days).round() as usize;
    let per_day = (1.0 / dt_days).round() as usize;
    let mut out = ResSimTrajectory {
        days: Vec::new(),
        max_composition_drift: 0.0,
        single_vapour: true,
        water_balance_error: 0.0,
    };
    for step in 1..=steps {
        let report = run.step(
            &spec, &layout, &rock, &relperm, &faces, &wells, &sources, dt_days, options,
        );
        assert_eq!(
            report.accepted_dt_days,
            Some(dt_days),
            "step {step}: {:?}",
            report.failure
        );
        for cell in run.state().cells() {
            let z = cell.overall_composition();
            for (zi, z0i) in z.iter().zip(z0) {
                out.max_composition_drift = out.max_composition_drift.max((zi - z0i).abs());
            }
            let f = flash(
                &spec,
                cell.pressure_bar * 1e5,
                spec.reservoir_temperature_k(),
                &z,
                None,
            )
            .unwrap();
            out.single_vapour &= f.phase_state == PhaseState::SingleVapour;
        }
        if step % per_day == 0 {
            let cells = run.state().cells();
            out.days.push((
                step as f64 * dt_days,
                cells.iter().map(|c| c.pressure_bar).collect(),
                cells.iter().map(|c| c.water_saturation).collect(),
            ));
        }
    }
    let after = totals(
        &run.accepted_inventory_with_water(&spec, &rock, water)
            .unwrap(),
    );
    let injected: f64 = run.cumulative_well_moles().iter().map(|w| w[9]).sum();
    out.water_balance_error = ((after[9] - initial[9]) - injected).abs() / initial[9];
    out
}

/// Worst `|Δp|` and `|ΔSw|` between a ResSim trajectory and a reference run, over every whole day.
fn worst_against(ours: &ResSimTrajectory, theirs: &ReferenceRun) -> (f64, f64) {
    let mut worst = (0.0f64, 0.0f64);
    for (day, p, sw) in &ours.days {
        let Some(reference) = theirs.reports.iter().find(|r| (r.day - day).abs() < 1e-9) else {
            continue;
        };
        for c in 0..p.len() {
            worst.0 = worst.0.max((p[c] - reference.p[c]).abs());
            worst.1 = worst.1.max((sw[c] - reference.sw[c]).abs());
        }
    }
    worst
}

/// The oracle is admissible: every run single-phase vapour, so the three-phase oil law never
/// enters, and successive refinements shrinking at first order, so it converges in time.
#[test]
fn comp_water_reference_oracle_converges_in_time() {
    let r = water_reference();
    assert_eq!(r.runs.len(), 4);
    for run in &r.runs {
        assert!(
            run.single_vapour,
            "a JutulDarcy run formed hydrocarbon liquid"
        );
        assert!(run.max_composition_drift < 1e-12);
    }
    assert_eq!(r.matched.dt_days, 1.0);
    assert_eq!(r.finest.dt_days, 0.25);
    assert_eq!(r.matched.reports.len(), 60);
    assert_eq!(r.finest.reports.len(), 60);
    // Backward Euler halves its error with the step. Each refinement's difference must shrink
    // by at least a third — the measured ratios are 0.62 and 0.56 in pressure, 0.64 and 0.51 in
    // Sw — or the "converged" reference is not.
    for pair in r.convergence.windows(2) {
        assert!(pair[1].max_dp_bar < 0.67 * pair[0].max_dp_bar);
        assert!(pair[1].max_dsw < 0.67 * pair[0].max_dsw);
    }
}

/// ResSim against JutulDarcy at the same report step, and at the reference's finest one.
///
/// Same grid, same backward Euler, same Δt, so the comparison measures model agreement and
/// nothing else. Measured on the committed fixture: 6.6e-6 bar and 1.7e-7 in Sw at Δt = 1 d, and
/// the same at 0.25 d — five orders below the oracle's own Δt = 1 → 0.25 change (0.34 bar,
/// 6.8e-3). The bands leave 15× and 60× headroom on those. Agreement at two step sizes is what
/// shows the two simulators converge to one limit rather than agree at one Δt.
///
/// The fine run covers the first 15 days only, where the injector's transient is largest; the
/// whole 60 days at Δt = 0.25 d would cost the debug gate several minutes for no added coverage.
///
/// The agreement depends on the oracle carrying no wellbore storage (see the script): with
/// JutulDarcy's default simple-well volume the injector first pushes its own gas, and the
/// comparison measured 1.3e-2 bar and 5.5e-4 in Sw instead, independent of Δt.
#[test]
fn comp_water_reference_matches_jutul_water_injection_into_gas() {
    let r = water_reference();
    for (dt, horizon, theirs) in [(1.0, r.horizon_days, &r.matched), (0.25, 15.0, &r.finest)] {
        let ours = run_water_reference(&r, dt, horizon);
        assert!(
            ours.single_vapour,
            "ResSim formed hydrocarbon liquid at dt = {dt}"
        );
        assert!(
            ours.max_composition_drift < 1e-12,
            "a single-phase gas changed composition by {:e}",
            ours.max_composition_drift
        );
        assert!(
            ours.water_balance_error < 1e-9,
            "water balance {:e}",
            ours.water_balance_error
        );
        let (dp, dsw) = worst_against(&ours, theirs);
        println!("water injection, dt = {dt} d: max |dp| {dp:e} bar, max |dSw| {dsw:e}");
        assert!(dp < 1e-4, "dt = {dt}: pressure mismatch {dp:e} bar");
        assert!(dsw < 1e-5, "dt = {dt}: water saturation mismatch {dsw:e}");
    }
}
