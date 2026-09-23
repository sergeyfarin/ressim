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
/// The same gap for free-gas saturation. Its absolute grid error (~2.7e-4 at nx 20→40) is the
/// same as before #11 made the table physical, but the physical table liberates less gas
/// (Sg ≈ 0.023 rather than 0.033), so the relative gap rose from 0.37% to 1.15% while the
/// contraction ratio stayed at ~0.53.
const FINEST_PAIR_TOLERANCE_SAT_GAS: f64 = 0.015;

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
    // Bo(100 bar) must keep the two-phase volume factor Bt = Bo + (Rs_i - Rs)·Bg falling with
    // pressure (`dBo/dp < Bg·dRs/dp`). At 1.05 it did not: below the bubble point the total
    // compressibility went negative until free gas built up, which IMPES cannot represent and
    // which cost it 144% of its oil balance (#11). `assert_pvt_is_thermodynamically_stable` guards it.
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 100.0,
                rs_m3m3: 5.0,
                bo_m3m3: 1.08,
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

fn assert_converging(name: &str, values: [f64; 4], finest_pair_tolerance: f64) {
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
        finest_pair_gap <= finest_pair_tolerance,
        "{} still differs by {:.3}% between the two finest grids (values {:?}), tolerance {:.3}%",
        name,
        finest_pair_gap * 100.0,
        values,
        finest_pair_tolerance * 100.0
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

/// Fails if the fixture's saturated curve lets the two-phase volume factor grow with pressure.
///
/// `dBt/dp = dBo/dp - Bg·dRs/dp` must be negative: otherwise oil plus the gas it liberates
/// occupies less volume as pressure falls, the total compressibility of a cell with little free
/// gas is negative, and the explicit IMPES pressure equation has no valid storage term. FIM and
/// Flow still run such a table, which is how the defect hid behind #11.
fn assert_pvt_is_thermodynamically_stable(sim: &ReservoirSimulator) {
    let table = sim
        .pvt_table
        .as_ref()
        .expect("black-oil fixture has a PVT table");
    let bubble_point = table.bubble_point_pressure(INITIAL_RS_SM3_SM3);
    let mut p = 100.5;
    while p < bubble_point - 0.5 {
        let (lo, mid, hi) = (
            table.interpolate(p - 0.5),
            table.interpolate(p),
            table.interpolate(p + 0.5),
        );
        let dbo_dp = hi.bo_m3m3 - lo.bo_m3m3;
        let bg_drs_dp = mid.bg_m3m3 * (hi.rs_m3m3 - lo.rs_m3m3);
        assert!(
            dbo_dp < bg_drs_dp,
            "PVT fixture is thermodynamically unstable at {p} bar: dBo/dp = {dbo_dp:.3e} >= Bg·dRs/dp = {bg_drs_dp:.3e}"
        );
        p += 1.0;
    }
}

/// Grid convergence on the IMPES path — fast enough to run as a default gate.
#[test]
fn physics_depletion_grid_convergence_impes() {
    assert_pvt_is_thermodynamically_stable(&make_black_oil_depletion_column_sim(5, false));
    let results: Vec<ColumnAverages> = REFINEMENT_LEVELS
        .iter()
        .map(|nx| run_column(*nx, false))
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

    assert_converging("average pressure", pressure, FINEST_PAIR_TOLERANCE);
    assert_converging("average Rs", rs, FINEST_PAIR_TOLERANCE);
    assert_converging("average Bo", bo, FINEST_PAIR_TOLERANCE);
    assert_converging(
        "average free-gas saturation",
        sat_gas,
        FINEST_PAIR_TOLERANCE_SAT_GAS,
    );
}

/// FIM twin of the IMPES check, held to the same contraction and finest-pair tolerances.
///
/// It used to be an `#[ignore]`d replay with only a 5% spread bound on Sg: before FIM-BUBBLE-001
/// the sweep took minutes and the substep ladder made the FIM Sg average non-monotone. It now
/// contracts like IMPES and runs in about a second in debug, so it is a default gate.
#[test]
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

    assert_converging("average pressure", pressure, FINEST_PAIR_TOLERANCE);
    assert_converging("average Rs", rs, FINEST_PAIR_TOLERANCE);
    assert_converging("average Bo", bo, FINEST_PAIR_TOLERANCE);

    assert_converging(
        "average free-gas saturation",
        sat_gas,
        FINEST_PAIR_TOLERANCE_SAT_GAS,
    );
}

/// Cumulative stock-tank oil production over the whole history [Sm³].
fn cumulative_oil_sm3(sim: &ReservoirSimulator) -> f64 {
    let mut previous_time = 0.0;
    let mut cumulative = 0.0;
    for point in &sim.rate_history {
        cumulative += point.total_production_oil * (point.time - previous_time);
        previous_time = point.time;
    }
    cumulative
}

/// #11/#37: IMPES and FIM agree on the depletion column, and IMPES conserves oil.
///
/// IMPES used to keep oil as the residual `So = 1 - Sw - Sg`, so a pressure equation that
/// misstated storage showed up as reported oil production the reservoir never lost: +144% on
/// the old unstable table, and +36% on a stable one at the default 75 bar pressure cap, where a
/// substep crossing the bubble point took the undersaturated storage term. It now transports oil
/// mass and iterates the pressure to the volume balance, so the error is roundoff at any cap.
/// Measured values: `docs/BLACK_OIL_VALIDATION.md` §2.
#[test]
fn physics_depletion_impes_matches_fim_and_conserves_oil() {
    let nx = 10;
    let mut impes = make_black_oil_depletion_column_sim(nx, false);
    let mut fim = make_black_oil_depletion_column_sim(nx, true);
    for _ in 0..STEPS {
        impes.step(DT_DAYS);
        fim.step(DT_DAYS);
    }

    let produced = cumulative_oil_sm3(&impes);
    let oil_error = impes
        .rate_history
        .last()
        .unwrap()
        .material_balance_error_oil_m3;
    assert!(
        oil_error.abs() <= 1e-8 * produced,
        "IMPES oil balance error {oil_error:.3e} Sm3 against {produced:.2} Sm3 produced"
    );

    let (impes, fim) = (column_averages(&impes), column_averages(&fim));
    println!(
        "nx={nx} oil MB {:+.2}% | IMPES p={:.4} sg={:.6} | FIM p={:.4} sg={:.6}",
        100.0 * oil_error / produced,
        impes.pressure_bar,
        impes.sat_gas,
        fim.pressure_bar,
        fim.sat_gas
    );
    assert!(
        (impes.pressure_bar - fim.pressure_bar).abs() <= 0.1,
        "IMPES p {:.4} vs FIM {:.4} bar",
        impes.pressure_bar,
        fim.pressure_bar
    );
    assert!(
        (impes.sat_gas - fim.sat_gas).abs() <= 0.005 * fim.sat_gas,
        "IMPES Sg {:.6} vs FIM {:.6}",
        impes.sat_gas,
        fim.sat_gas
    );
}

/// #37: three-phase IMPES closes all three component balances with compressible rock and water.
///
/// The pressure equation has always counted rock and water expansion in `c_t`, but transport
/// moved water by reservoir volume on a fixed pore volume, and oil was the residual
/// `1 - Sw - Sg`, so both expansions were produced as oil the reservoir never lost. Each
/// balance is now exact up to the volume-balance tolerance.
#[test]
fn physics_depletion_impes_closes_balances_with_rock_and_water_compressibility() {
    let mut sim = make_black_oil_depletion_column_sim(10, false);
    sim.rock_compressibility = 1e-4;
    sim.pvt.c_w = 4.5e-5;
    for _ in 0..STEPS {
        sim.step(DT_DAYS);
        assert!(sim.last_solver_warning.is_empty(), "{}", sim.last_solver_warning);
    }

    let produced_oil = cumulative_oil_sm3(&sim);
    let last = sim.rate_history.last().unwrap();
    let water_in_place: f64 = (0..sim.nx).map(|id| sim.cell_masses(id).water_sc).sum();
    let gas_in_place: f64 = (0..sim.nx).map(|id| sim.cell_masses(id).total_gas_sc()).sum();
    assert!(
        last.material_balance_error_oil_m3 <= 1e-8 * produced_oil,
        "oil balance error {:.3e} Sm3 against {produced_oil:.2} produced",
        last.material_balance_error_oil_m3
    );
    assert!(
        last.material_balance_error_m3 <= 1e-8 * water_in_place,
        "water balance error {:.3e} Sm3 against {water_in_place:.2} in place",
        last.material_balance_error_m3
    );
    assert!(
        last.material_balance_error_gas_m3 <= 1e-8 * gas_in_place,
        "gas balance error {:.3e} Sm3 against {gas_in_place:.2} in place",
        last.material_balance_error_gas_m3
    );
}

/// FIM-BUBBLE-001: the first report step crosses the bubble point. It used to take 21,797
/// substeps at nx=10 (19,008 at nx=40), because gas-free cells were carried on an Sg primary
/// pinned to the Sg = 0 relperm kink and never switched to Rs. OPM Flow takes 4 on the same
/// column (`opm/reference-decks/small-direct/bo-1d-10`).
#[test]
fn fim_bubble_point_crossing_does_not_fragment() {
    for nx in [10, 40] {
        let mut sim = make_black_oil_depletion_column_sim(nx, true);
        sim.step(DT_DAYS);
        assert!(
            sim.last_solver_warning.is_empty(),
            "nx={nx}: {}",
            sim.last_solver_warning
        );
        assert!(
            (sim.time_days - DT_DAYS).abs() < 1e-9,
            "nx={nx}: horizon not completed"
        );
        assert!(
            sim.rate_history.len() <= 12,
            "nx={nx}: the bubble-point step took {} substeps; Flow takes 4",
            sim.rate_history.len()
        );
        assert!(
            sim.sat_gas.iter().any(|&sg| sg > 1e-3),
            "nx={nx}: no gas was liberated"
        );
    }
}
