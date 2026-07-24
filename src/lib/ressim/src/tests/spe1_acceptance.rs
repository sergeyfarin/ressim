//! Quantitative acceptance criteria for the SPE1 black-oil comparative-solution case.
//!
//! Reference: SPE Comparative Solution Project #1 (Odeh, 1981, SPE 9723), Case 1.
//! The numbers below are the same reference series the frontend overlays on the
//! `spe1_gas_injection` scenario, produced offline by `flow 2026.04` on
//! `OPM/opm-common/tests/SPE1CASE1.DATA` (see `TODO.md`, "SPE1 reference data (2026-07-24)"
//! and `docs/BLACK_OIL_VALIDATION.md`). Keeping them here lets the Rust engine be graded
//! against the published case without going through the frontend.
//!
//! Tolerances are acceptance criteria, not benchmark tolerances tuned to the current build:
//! each one is documented in `docs/BLACK_OIL_VALIDATION.md` together with the error actually
//! measured at the recorded baseline. Do not widen one to make a change pass.

use crate::ReservoirSimulator;
use crate::tests::make_spe1_like_grid_sim;

/// Field average reservoir pressure [bar] at yearly report times.
const REF_PRESSURE_BAR: [(f64, f64); 10] = [
    (365.0, 397.9),
    (730.0, 441.6),
    (1095.0, 404.1),
    (1460.0, 355.5),
    (1825.0, 323.4),
    (2190.0, 302.0),
    (2555.0, 285.8),
    (2920.0, 273.9),
    (3285.0, 265.3),
    (3650.0, 256.9),
];

/// Producing gas-oil ratio [Sm³/Sm³] at yearly report times.
const REF_GOR: [(f64, f64); 10] = [
    (365.0, 219.1),
    (730.0, 393.7),
    (1095.0, 1328.5),
    (1460.0, 1638.5),
    (1825.0, 1866.6),
    (2190.0, 2091.0),
    (2555.0, 2392.1),
    (2920.0, 2837.6),
    (3285.0, 3318.3),
    (3650.0, 3824.0),
];

/// Producer surface oil rate [Sm³/day]; sampled on the reference report schedule and
/// interpolated onto the checkpoints below.
const REF_OIL_RATE: [(f64, f64); 45] = [
    (1.0, 3179.7),
    (12.0, 3179.7),
    (59.0, 3179.7),
    (120.0, 3179.7),
    (181.0, 3179.7),
    (243.0, 3179.7),
    (304.0, 3179.7),
    (365.0, 3179.7),
    (424.0, 3179.7),
    (485.0, 3179.7),
    (546.0, 3179.7),
    (608.0, 3179.7),
    (669.0, 3179.7),
    (730.0, 3179.7),
    (789.0, 3179.7),
    (850.0, 3179.7),
    (911.0, 3179.7),
    (973.0, 3179.7),
    (1034.0, 2982.1),
    (1095.0, 2791.5),
    (1154.0, 2639.1),
    (1215.0, 2507.1),
    (1276.0, 2396.6),
    (1338.0, 2297.8),
    (1399.0, 2211.0),
    (1460.0, 2132.2),
    (1519.0, 2060.0),
    (1641.0, 1921.9),
    (1764.0, 1806.3),
    (1884.0, 1715.4),
    (2006.0, 1642.8),
    (2129.0, 1573.3),
    (2249.0, 1474.4),
    (2371.0, 1410.7),
    (2494.0, 1347.9),
    (2614.0, 1281.8),
    (2736.0, 1213.8),
    (2859.0, 1155.2),
    (2979.0, 1105.2),
    (3101.0, 1060.2),
    (3224.0, 1016.6),
    (3344.0, 975.5),
    (3466.0, 937.7),
    (3589.0, 901.9),
    (3650.0, 883.7),
];

/// Deck surface targets: 20,000 STB/day producer oil, 100 MMscf/day gas injection.
const TARGET_OIL_RATE_SC_DAY: f64 = 3179.74;
const TARGET_GAS_INJECTION_SC_DAY: f64 = 2_831_680.0;

/// Acceptance tolerances (relative). See `docs/BLACK_OIL_VALIDATION.md`.
const PRESSURE_TOLERANCE: f64 = 0.03;
const OIL_RATE_TOLERANCE: f64 = 0.08;
const GOR_TOLERANCE: f64 = 0.12;
/// The producer is on its oil-rate target until the reference series leaves plateau (973 days).
const PLATEAU_TOLERANCE: f64 = 0.005;
/// Oil material-balance drift, relative to stock-tank oil initially in place.
const OIL_MATERIAL_BALANCE_TOLERANCE: f64 = 0.01;
/// Gas material-balance drift, relative to total surface gas handled by the case
/// (initial free + dissolved gas in place, plus cumulative injection to date).
const GAS_MATERIAL_BALANCE_TOLERANCE: f64 = 0.01;

fn interpolate(series: &[(f64, f64)], t_days: f64) -> f64 {
    if t_days <= series[0].0 {
        return series[0].1;
    }
    for window in series.windows(2) {
        let (t0, v0) = window[0];
        let (t1, v1) = window[1];
        if t_days <= t1 {
            return v0 + (v1 - v0) * (t_days - t0) / (t1 - t0);
        }
    }
    series[series.len() - 1].1
}

/// SPE1 Case 1 at an arbitrary areal resolution over the same 3048 m × 3048 m domain, with the
/// producer in the far corner. FIM and the stability caps used by the `spe1_gas_injection`
/// scenario.
fn make_spe1_acceptance_sim_at(nx: usize) -> ReservoirSimulator {
    let mut sim = make_spe1_like_grid_sim(
        nx,
        nx,
        nx - 1,
        nx - 1,
        vec![500.0, 50.0, 200.0],
        0.05,
        20.0,
        0.2,
    );
    sim.set_fim_enabled(true);
    sim
}

/// SPE1 Case 1 on the catalog's shipped configuration: 10×10×3.
fn make_spe1_acceptance_sim() -> ReservoirSimulator {
    make_spe1_acceptance_sim_at(10)
}

/// Stock-tank oil initially in place [Sm³], used as the material-balance denominator.
fn stock_tank_oil_in_place_sm3(sim: &ReservoirSimulator) -> f64 {
    (0..sim.nx * sim.ny * sim.nz)
        .map(|id| {
            let bo = sim.get_b_o_cell(id, sim.pressure[id]).max(1e-9);
            sim.sat_oil[id] * sim.pore_volume_m3(id) / bo
        })
        .sum()
}

/// Total gas initially in place [Sm³], free plus dissolved.
fn gas_in_place_sm3(sim: &ReservoirSimulator) -> f64 {
    (0..sim.nx * sim.ny * sim.nz)
        .map(|id| {
            let pore_volume_m3 = sim.pore_volume_m3(id);
            let bg = sim.get_b_g(sim.pressure[id]).max(1e-9);
            let bo = sim.get_b_o_cell(id, sim.pressure[id]).max(1e-9);
            sim.sat_gas[id] * pore_volume_m3 / bg
                + sim.sat_oil[id] * pore_volume_m3 / bo * sim.rs[id]
        })
        .sum()
}

/// Advance to exactly `target_days` using report steps of at most `max_dt_days`.
fn step_to(sim: &mut ReservoirSimulator, target_days: f64, max_dt_days: f64) {
    while sim.time_days < target_days - 1e-9 {
        let dt = max_dt_days.min(target_days - sim.time_days);
        sim.step(dt);
        assert!(
            sim.last_solver_warning.is_empty(),
            "SPE1 acceptance run emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }
}

struct CheckpointErrors {
    pressure: f64,
    oil_rate: f64,
    gor: f64,
}

fn checkpoint_errors(sim: &ReservoirSimulator, t_days: f64) -> CheckpointErrors {
    let point = sim
        .rate_history
        .last()
        .expect("rate history should have a reported point");

    CheckpointErrors {
        pressure: (point.avg_reservoir_pressure - interpolate(&REF_PRESSURE_BAR, t_days)).abs()
            / interpolate(&REF_PRESSURE_BAR, t_days),
        oil_rate: (point.total_production_oil - interpolate(&REF_OIL_RATE, t_days)).abs()
            / interpolate(&REF_OIL_RATE, t_days),
        gor: (point.producing_gor - interpolate(&REF_GOR, t_days)).abs()
            / interpolate(&REF_GOR, t_days),
    }
}

fn check_checkpoint(sim: &ReservoirSimulator, t_days: f64) -> CheckpointErrors {
    let point = sim
        .rate_history
        .last()
        .expect("rate history should have a reported point");

    let ref_pressure = interpolate(&REF_PRESSURE_BAR, t_days);
    let ref_oil_rate = interpolate(&REF_OIL_RATE, t_days);
    let ref_gor = interpolate(&REF_GOR, t_days);

    let CheckpointErrors {
        pressure: pressure_error,
        oil_rate: oil_rate_error,
        gor: gor_error,
    } = checkpoint_errors(sim, t_days);

    assert!(
        pressure_error <= PRESSURE_TOLERANCE,
        "SPE1 average reservoir pressure outside acceptance band at t={}: got {:.2} bar, reference {:.2} bar, error {:.2}% > {:.2}%",
        t_days,
        point.avg_reservoir_pressure,
        ref_pressure,
        pressure_error * 100.0,
        PRESSURE_TOLERANCE * 100.0
    );
    assert!(
        oil_rate_error <= OIL_RATE_TOLERANCE,
        "SPE1 producer oil rate outside acceptance band at t={}: got {:.2} Sm3/d, reference {:.2} Sm3/d, error {:.2}% > {:.2}%",
        t_days,
        point.total_production_oil,
        ref_oil_rate,
        oil_rate_error * 100.0,
        OIL_RATE_TOLERANCE * 100.0
    );
    assert!(
        gor_error <= GOR_TOLERANCE,
        "SPE1 producing GOR outside acceptance band at t={}: got {:.2} Sm3/Sm3, reference {:.2} Sm3/Sm3, error {:.2}% > {:.2}%",
        t_days,
        point.producing_gor,
        ref_gor,
        gor_error * 100.0,
        GOR_TOLERANCE * 100.0
    );

    CheckpointErrors {
        pressure: pressure_error,
        oil_rate: oil_rate_error,
        gor: gor_error,
    }
}

fn assert_material_balance(sim: &ReservoirSimulator, stoiip_sm3: f64, gas_in_place_sm3: f64) {
    let point = sim.rate_history.last().expect("rate history");
    let oil_drift = point.material_balance_error_oil_m3.abs() / stoiip_sm3;
    assert!(
        oil_drift <= OIL_MATERIAL_BALANCE_TOLERANCE,
        "SPE1 oil material-balance drift too large at t={}: {:.1} Sm3 = {:.3}% of STOIIP ({:.1} Sm3) > {:.3}%",
        sim.time_days,
        point.material_balance_error_oil_m3,
        oil_drift * 100.0,
        stoiip_sm3,
        OIL_MATERIAL_BALANCE_TOLERANCE * 100.0
    );

    let gas_handled_sc = gas_in_place_sm3 + TARGET_GAS_INJECTION_SC_DAY * sim.time_days;
    let gas_drift = point.material_balance_error_gas_m3.abs() / gas_handled_sc;
    assert!(
        gas_drift <= GAS_MATERIAL_BALANCE_TOLERANCE,
        "SPE1 gas material-balance drift too large at t={}: {:.1} Sm3 = {:.3}% of gas handled ({:.1} Sm3) > {:.3}%",
        sim.time_days,
        point.material_balance_error_gas_m3,
        gas_drift * 100.0,
        gas_handled_sc,
        GAS_MATERIAL_BALANCE_TOLERANCE * 100.0
    );
}

/// Fast gate: the first reference year, where the producer must hold its surface-rate target,
/// pressure must follow the injection-supported rise, and the oil must stay saturated-free
/// enough that producing GOR is still the solution GOR.
#[test]
fn spe1_first_year_matches_published_reference() {
    let mut sim = make_spe1_acceptance_sim();
    let stoiip_sm3 = stock_tank_oil_in_place_sm3(&sim);
    let gas_in_place_sm3 = gas_in_place_sm3(&sim);

    step_to(&mut sim, 365.0, 30.0);

    let point = sim.rate_history.last().expect("rate history");
    let plateau_error =
        (point.total_production_oil - TARGET_OIL_RATE_SC_DAY).abs() / TARGET_OIL_RATE_SC_DAY;
    assert!(
        plateau_error <= PLATEAU_TOLERANCE,
        "SPE1 producer should still be on its 20,000 STB/d surface target at t=365: got {:.2} Sm3/d ({:.3}% off)",
        point.total_production_oil,
        plateau_error * 100.0
    );

    check_checkpoint(&sim, 365.0);
    assert_material_balance(&sim, stoiip_sm3, gas_in_place_sm3);
}

/// Full 10-year acceptance replay against the published Case 1 series.
///
/// Ignored by default: ~3 s in release, ~90 s in debug. Replay with
/// `cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
///  spe1_full_horizon_matches_published_reference -- --ignored --nocapture`.
#[test]
#[ignore = "acceptance replay: 3652-day SPE1 run, use --release (see docs/BLACK_OIL_VALIDATION.md)"]
fn spe1_full_horizon_matches_published_reference() {
    let mut sim = make_spe1_acceptance_sim();
    let stoiip_sm3 = stock_tank_oil_in_place_sm3(&sim);
    let gas_in_place_sm3 = gas_in_place_sm3(&sim);

    let mut worst = CheckpointErrors {
        pressure: 0.0,
        oil_rate: 0.0,
        gor: 0.0,
    };

    for (t_days, _) in REF_PRESSURE_BAR {
        step_to(&mut sim, t_days, 30.0);

        if t_days <= 730.0 {
            let point = sim.rate_history.last().expect("rate history");
            let plateau_error = (point.total_production_oil - TARGET_OIL_RATE_SC_DAY).abs()
                / TARGET_OIL_RATE_SC_DAY;
            assert!(
                plateau_error <= PLATEAU_TOLERANCE,
                "SPE1 producer left its surface-rate plateau before the reference did (t={}): got {:.2} Sm3/d",
                t_days,
                point.total_production_oil
            );
        }

        let errors = check_checkpoint(&sim, t_days);
        assert_material_balance(&sim, stoiip_sm3, gas_in_place_sm3);

        println!(
            "t={:7.1} pressure_err={:6.3}% oil_rate_err={:6.3}% gor_err={:6.3}%",
            t_days,
            errors.pressure * 100.0,
            errors.oil_rate * 100.0,
            errors.gor * 100.0
        );
        worst.pressure = worst.pressure.max(errors.pressure);
        worst.oil_rate = worst.oil_rate.max(errors.oil_rate);
        worst.gor = worst.gor.max(errors.gor);
    }

    println!(
        "SPE1 worst-case errors: pressure={:.3}% oil_rate={:.3}% gor={:.3}%",
        worst.pressure * 100.0,
        worst.oil_rate * 100.0,
        worst.gor * 100.0
    );
}

/// Characterization replay: how reference agreement changes when the same case is refined
/// arealy (the catalog's `grid` sensitivity offers 20×20×3). Refinement is *not* currently a
/// pass/fail criterion — the refined grid breaks through early and its producing GOR moves away
/// from the reference — so this run asserts only that the case stays clean and closes material
/// balance, and prints the errors that `docs/BLACK_OIL_VALIDATION.md` records.
///
/// `cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
///  spe1_areal_refinement_reference_error_replay -- --ignored --nocapture`
#[test]
#[ignore = "characterization replay: 3650-day SPE1 at 10x10x3 and 20x20x3, use --release"]
fn spe1_areal_refinement_reference_error_replay() {
    for nx in [10usize, 20] {
        let mut sim = make_spe1_acceptance_sim_at(nx);
        let stoiip_sm3 = stock_tank_oil_in_place_sm3(&sim);
        let gas_in_place_sm3 = gas_in_place_sm3(&sim);

        for (t_days, _) in REF_PRESSURE_BAR {
            step_to(&mut sim, t_days, 30.0);
            assert_material_balance(&sim, stoiip_sm3, gas_in_place_sm3);

            let errors = checkpoint_errors(&sim, t_days);
            println!(
                "nx={:3} t={:7.1} pressure_err={:7.3}% oil_rate_err={:7.3}% gor_err={:7.3}%",
                nx,
                t_days,
                errors.pressure * 100.0,
                errors.oil_rate * 100.0,
                errors.gor * 100.0
            );
        }
    }
}
