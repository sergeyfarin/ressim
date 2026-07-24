//! Grid-convergence checks for black-oil depletion through the bubble point.
//!
//! A 1D column is depleted from an undersaturated initial state (175 bar) to a producer BHP
//! below the bubble point (150 bar), so pressure, dissolved gas `Rs`, oil formation volume
//! factor `Bo`, and liberated free gas `Sg` are all active. The same physical domain is
//! discretized at 5/10/20/40 cells; the pore-volume-weighted field averages must form a
//! converging sequence rather than drifting with resolution.
//!
//! Measured baselines and the replay commands are recorded in `docs/BLACK_OIL_VALIDATION.md`.

use crate::ReservoirSimulator;
use crate::pvt::{PvtRow, PvtTable};

const COLUMN_LENGTH_M: f64 = 1000.0;
const COLUMN_WIDTH_M: f64 = 200.0;
const COLUMN_HEIGHT_M: f64 = 20.0;
const INITIAL_PRESSURE_BAR: f64 = 175.0;
const BUBBLE_POINT_BAR: f64 = 150.0;
const INITIAL_RS_SM3_SM3: f64 = 15.0;
const PRODUCER_BHP_BAR: f64 = 120.0;

const REFINEMENT_LEVELS: [usize; 4] = [5, 10, 20, 40];
const DT_DAYS: f64 = 5.0;
const STEPS: usize = 20;

/// Successive refinement differences must shrink by at least this factor. First-order upstream
/// transport gives ~0.5-0.6 in the recorded baseline; 0.8 leaves headroom without admitting a
/// non-converging sequence.
const CONTRACTION_RATIO: f64 = 0.8;
/// Relative gap allowed between the two finest grids.
const FINEST_PAIR_TOLERANCE: f64 = 0.01;

fn make_black_oil_depletion_column_sim(nx: usize, fim_enabled: bool) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(nx, 1, 1, 0.2);
    sim.set_fim_enabled(fim_enabled);
    sim.set_cell_dimensions_per_layer(
        COLUMN_LENGTH_M / nx as f64,
        COLUMN_WIDTH_M,
        vec![COLUMN_HEIGHT_M],
    )
    .unwrap();
    sim.set_permeability_per_layer(vec![100.0], vec![100.0], vec![100.0])
        .unwrap();
    sim.set_three_phase_rel_perm_props(0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7)
        .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_gas_redissolution_enabled(false);
    sim.set_gravity_enabled(false);
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_initial_pressure(INITIAL_PRESSURE_BAR);
    sim.set_initial_saturation(0.10);
    sim.set_initial_gas_saturation(0.0);
    sim.pvt.c_o = 1e-5;
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 100.0,
                rs_m3m3: 5.0,
                bo_m3m3: 1.05,
                mu_o_cp: 1.5,
                bg_m3m3: 0.01,
                mu_g_cp: 0.02,
            },
            PvtRow {
                p_bar: BUBBLE_POINT_BAR,
                rs_m3m3: INITIAL_RS_SM3_SM3,
                bo_m3m3: 1.12,
                mu_o_cp: 1.2,
                bg_m3m3: 0.006,
                mu_g_cp: 0.025,
            },
            PvtRow {
                p_bar: 200.0,
                rs_m3m3: INITIAL_RS_SM3_SM3,
                bo_m3m3: 1.119,
                mu_o_cp: 1.3,
                bg_m3m3: 0.0045,
                mu_g_cp: 0.03,
            },
        ],
        sim.pvt.c_o,
    ));
    sim.set_initial_rs(INITIAL_RS_SM3_SM3);
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.injector_enabled = false;
    sim.add_well(nx - 1, 0, 0, PRODUCER_BHP_BAR, 0.1, 0.0, false)
        .unwrap();
    sim
}

/// Pore-volume-weighted field averages of the four black-oil state variables.
struct ColumnAverages {
    pressure_bar: f64,
    rs_sm3_sm3: f64,
    bo_m3_sm3: f64,
    sat_gas: f64,
}

fn column_averages(sim: &ReservoirSimulator) -> ColumnAverages {
    let mut pore_volume_sum = 0.0;
    let mut pressure = 0.0;
    let mut rs = 0.0;
    let mut bo = 0.0;
    let mut sat_gas = 0.0;
    for id in 0..sim.nx * sim.ny * sim.nz {
        let pore_volume_m3 = sim.pore_volume_m3(id);
        pore_volume_sum += pore_volume_m3;
        pressure += pore_volume_m3 * sim.pressure[id];
        rs += pore_volume_m3 * sim.rs[id];
        bo += pore_volume_m3 * sim.get_b_o_cell(id, sim.pressure[id]);
        sat_gas += pore_volume_m3 * sim.sat_gas[id];
    }
    ColumnAverages {
        pressure_bar: pressure / pore_volume_sum,
        rs_sm3_sm3: rs / pore_volume_sum,
        bo_m3_sm3: bo / pore_volume_sum,
        sat_gas: sat_gas / pore_volume_sum,
    }
}

fn run_column(nx: usize, fim_enabled: bool) -> ColumnAverages {
    let mut sim = make_black_oil_depletion_column_sim(nx, fim_enabled);
    for _ in 0..STEPS {
        sim.step(DT_DAYS);
        assert!(
            sim.last_solver_warning.is_empty(),
            "grid-convergence column nx={} emitted solver warning at t={}: {}",
            nx,
            sim.time_days,
            sim.last_solver_warning
        );
    }
    column_averages(&sim)
}

fn refinement_series(results: &[ColumnAverages], select: fn(&ColumnAverages) -> f64) -> [f64; 4] {
    [
        select(&results[0]),
        select(&results[1]),
        select(&results[2]),
        select(&results[3]),
    ]
}

fn assert_converging(name: &str, values: [f64; 4]) {
    for value in values {
        assert!(
            value.is_finite(),
            "{} produced a non-finite field average: {:?}",
            name,
            values
        );
    }

    let coarse_diff = (values[1] - values[0]).abs();
    let medium_diff = (values[2] - values[1]).abs();
    let fine_diff = (values[3] - values[2]).abs();

    assert!(
        medium_diff <= coarse_diff * CONTRACTION_RATIO,
        "{} is not converging under refinement (5→10 diff {:.6e}, 10→20 diff {:.6e}, values {:?})",
        name,
        coarse_diff,
        medium_diff,
        values
    );
    assert!(
        fine_diff <= medium_diff * CONTRACTION_RATIO,
        "{} is not converging under refinement (10→20 diff {:.6e}, 20→40 diff {:.6e}, values {:?})",
        name,
        medium_diff,
        fine_diff,
        values
    );

    let finest_pair_gap = fine_diff / values[3].abs().max(1e-12);
    assert!(
        finest_pair_gap <= FINEST_PAIR_TOLERANCE,
        "{} still differs by {:.3}% between the two finest grids (values {:?}), tolerance {:.3}%",
        name,
        finest_pair_gap * 100.0,
        values,
        FINEST_PAIR_TOLERANCE * 100.0
    );
}

fn assert_case_actually_liberates_gas(pressure: [f64; 4], rs: [f64; 4], sat_gas: [f64; 4]) {
    assert!(
        pressure[3] < BUBBLE_POINT_BAR,
        "column should have dropped below the bubble point, average pressure {:.2} bar",
        pressure[3]
    );
    assert!(
        rs[3] < INITIAL_RS_SM3_SM3,
        "dissolved gas should have come out of solution, average Rs {:.4} Sm3/Sm3",
        rs[3]
    );
    assert!(
        sat_gas[3] > 1e-3,
        "liberation should leave measurable free gas, average Sg {:.6}",
        sat_gas[3]
    );
}

/// Grid convergence on the IMPES path — fast enough to run as a default gate.
#[test]
fn physics_depletion_grid_convergence_impes() {
    let results: Vec<ColumnAverages> = REFINEMENT_LEVELS
        .iter()
        .map(|nx| run_column(*nx, false))
        .collect();

    let pressure = refinement_series(&results, |values| values.pressure_bar);
    let rs = refinement_series(&results, |values| values.rs_sm3_sm3);
    let bo = refinement_series(&results, |values| values.bo_m3_sm3);
    let sat_gas = refinement_series(&results, |values| values.sat_gas);

    assert_case_actually_liberates_gas(pressure, rs, sat_gas);

    assert_converging("average pressure", pressure);
    assert_converging("average Rs", rs);
    assert_converging("average Bo", bo);
    assert_converging("average free-gas saturation", sat_gas);
}

/// FIM twin of the IMPES check. FIM is the shipped solver for three-phase scenarios, but the
/// four-level refinement sweep costs minutes in release, so it is an explicit replay rather
/// than a default gate:
/// `cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
///  physics_depletion_grid_convergence_fim -- --ignored --nocapture`
///
/// Free-gas saturation is only checked for a bounded spread here: unlike IMPES, the FIM substep
/// ladder makes the liberated-gas average non-monotone at the 1e-4 level (recorded in
/// `docs/BLACK_OIL_VALIDATION.md`), which is well inside the physical signal but outside what a
/// strict contraction test admits.
#[test]
#[ignore = "grid-convergence replay: 4-level FIM refinement sweep, use --release"]
fn physics_depletion_grid_convergence_fim() {
    let results: Vec<ColumnAverages> = REFINEMENT_LEVELS
        .iter()
        .map(|nx| run_column(*nx, true))
        .collect();

    for (nx, values) in REFINEMENT_LEVELS.iter().zip(results.iter()) {
        println!(
            "nx={:3} pressure={:9.4} rs={:9.5} bo={:9.6} sg={:9.6}",
            nx, values.pressure_bar, values.rs_sm3_sm3, values.bo_m3_sm3, values.sat_gas
        );
    }

    let pressure = refinement_series(&results, |values| values.pressure_bar);
    let rs = refinement_series(&results, |values| values.rs_sm3_sm3);
    let bo = refinement_series(&results, |values| values.bo_m3_sm3);
    let sat_gas = refinement_series(&results, |values| values.sat_gas);

    assert_case_actually_liberates_gas(pressure, rs, sat_gas);

    assert_converging("average pressure", pressure);
    assert_converging("average Rs", rs);
    assert_converging("average Bo", bo);

    let finest = sat_gas[3];
    for (nx, value) in REFINEMENT_LEVELS.iter().zip(sat_gas.iter()) {
        let spread = (value - finest).abs() / finest.abs().max(1e-12);
        assert!(
            spread <= 0.05,
            "FIM free-gas saturation drifts with refinement: nx={} Sg={:.6} vs finest {:.6} ({:.2}%)",
            nx,
            value,
            finest,
            spread * 100.0
        );
    }
}
