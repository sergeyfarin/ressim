//! Quantitative acceptance criteria for three-phase (oil/water/gas) behavior.
//!
//! This is the three-phase companion to `spe1_acceptance.rs`. Where that file grades the
//! black-oil gas-*injection* path against SPE1, this one covers the two things SPE1 alone
//! does not pin down:
//!
//! 1. **Solution gas drive against OPM Flow.** The `gas_drive` catalog scenario is
//!    reproduced here cell-for-cell and graded against a `flow 2026.04` run of the deck in
//!    `tools/opm_flow/opm_flow_tool/cases.py::GAS_DRIVE`, whose parsed series are also
//!    committed as `src/lib/catalog/opm-flow-results/gas_drive.json`.
//! 2. **Gas-front behavior in a gas flood** — breakthrough timing, gas-saturation
//!    evolution, and per-phase material-balance closure including the oil phase.
//!
//! Tolerances are acceptance criteria with deliberate headroom, not benchmark tolerances
//! tuned to the current build. Each is documented in `docs/THREE_PHASE_VALIDATION.md`
//! alongside the error actually measured at the recorded baseline. Do not widen one to make
//! a change pass; a regression that breaks one is a physics or solver finding.

use crate::ReservoirSimulator;
use crate::pvt::{PvtRow, PvtTable};
use crate::tests::physics::fixtures::{
    make_3phase_gas_injection_sim, total_gas_inventory_sc_all_cells,
};

// ─── gas_drive: scenario mirror ──────────────────────────────────────────────

/// Bubble point of the `gas_drive` PVT table [bar]. Equal to the initial pressure, so the
/// reservoir starts saturated and drawdown liberates gas immediately.
const GAS_DRIVE_BUBBLE_POINT_BAR: f64 = 200.0;
/// Solution GOR at the bubble point [Sm³/Sm³].
const GAS_DRIVE_INITIAL_RS: f64 = 28.24377;
const GAS_DRIVE_PRODUCER_BHP_BAR: f64 = 100.0;
const GAS_DRIVE_INITIAL_SW: f64 = 0.2;
const GAS_DRIVE_INITIAL_SG: f64 = 0.08;

/// The `gas_drive` scenario's PVT table, i.e. the output of
/// `generateBlackOilTable(35 API, 0.75 gas gravity, 80 C, Pb = 200 bar, Pmax = 300 bar,
/// 20 points, c_o = 1e-5/bar)`. The same 20 rows are emitted verbatim into the OPM deck's
/// PVTO/PVDG, so engine and reference read identical fluid properties.
fn gas_drive_pvt_rows() -> Vec<PvtRow> {
    const ROWS: [(f64, f64, f64, f64, f64, f64); 20] = [
        (1.0000, 0.04771, 1.05365, 2.27850, 1.239768, 0.01254),
        (15.7895, 1.32560, 1.05651, 2.17064, 0.076466, 0.01271),
        (31.5789, 3.05563, 1.06041, 2.04252, 0.037254, 0.01299),
        (47.3684, 4.98034, 1.06479, 1.91951, 0.024222, 0.01337),
        (63.1579, 7.04348, 1.06953, 1.80582, 0.017742, 0.01382),
        (78.9474, 9.21608, 1.07456, 1.70224, 0.013889, 0.01436),
        (94.7368, 11.48009, 1.07984, 1.60842, 0.011355, 0.01498),
        (110.5263, 13.82306, 1.08536, 1.52356, 0.009579, 0.01568),
        (126.3158, 16.23581, 1.09109, 1.44676, 0.008280, 0.01646),
        (142.1053, 18.71128, 1.09702, 1.37711, 0.007300, 0.01731),
        (157.8947, 21.24384, 1.10314, 1.31381, 0.006546, 0.01821),
        (173.6842, 23.82889, 1.10944, 1.25610, 0.005954, 0.01916),
        (189.4737, 26.46258, 1.11590, 1.20334, 0.005483, 0.02014),
        (200.0000, 28.24377, 1.12030, 1.17063, 0.005220, 0.02081),
        (221.0526, 28.24377, 1.12007, 1.20424, 0.004791, 0.02214),
        (236.8421, 28.24377, 1.11989, 1.23168, 0.004534, 0.02313),
        (252.6316, 28.24377, 1.11971, 1.26092, 0.004318, 0.02412),
        (268.4211, 28.24377, 1.11953, 1.29182, 0.004135, 0.02508),
        (284.2105, 28.24377, 1.11936, 1.32430, 0.003978, 0.02603),
        (300.0000, 28.24377, 1.11918, 1.35827, 0.003842, 0.02695),
    ];

    ROWS.iter()
        .map(
            |&(p_bar, rs_m3m3, bo_m3m3, mu_o_cp, bg_m3m3, mu_g_cp)| PvtRow {
                p_bar,
                rs_m3m3,
                bo_m3m3,
                mu_o_cp,
                bg_m3m3,
                mu_g_cp,
            },
        )
        .collect()
}

/// The `gas_drive` catalog scenario, rebuilt through the engine's public API: 20×1×1 slab of
/// 50 m × 50 m × 10 m cells, 100 mD, porosity 0.2, saturated at 200 bar with 8 % free gas, a
/// single BHP-controlled producer in the last cell and no injector. Gravity is off, matching
/// both the scenario and the flat single-layer deck.
fn make_gas_drive_acceptance_sim() -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(20, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(50.0, 50.0, vec![10.0])
        .unwrap();
    sim.set_permeability_per_layer(vec![100.0], vec![100.0], vec![10.0])
        .unwrap();
    // mu_o is the two-phase fallback only: with a PVT table present the engine reads oil
    // viscosity from the table (pvt.rs::get_mu_o_cell), exactly as the deck does.
    sim.set_fluid_properties(2.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    // Surface gas density for 0.75 gas gravity — the DENSITY value in the deck.
    sim.set_gas_fluid_properties(0.02, 1e-4, 0.9172).unwrap();
    sim.set_rock_properties(1e-6, 0.2, 1.0, 1.0).unwrap();
    sim.pvt_table = Some(PvtTable::new(gas_drive_pvt_rows(), sim.pvt.c_o));
    sim.set_initial_rs(GAS_DRIVE_INITIAL_RS);
    sim.set_initial_pressure(GAS_DRIVE_BUBBLE_POINT_BAR);
    sim.set_initial_saturation(GAS_DRIVE_INITIAL_SW);
    sim.set_three_phase_rel_perm_props(
        GAS_DRIVE_INITIAL_SW,
        0.15,
        0.05,
        0.05,
        0.20,
        2.0,
        2.0,
        1.5,
        0.4,
        1.0,
        0.8,
    )
    .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_initial_gas_saturation(GAS_DRIVE_INITIAL_SG);
    sim.set_gas_redissolution_enabled(true);
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.pc_og = None;
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.1, 75.0, 0.75);
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.injector_enabled = false;
    sim.add_well(19, 0, 0, GAS_DRIVE_PRODUCER_BHP_BAR, 0.1, 0.0, false)
        .unwrap();
    sim
}

fn step_to(sim: &mut ReservoirSimulator, target_days: f64, max_dt_days: f64) {
    while sim.time_days < target_days - 1e-9 {
        let dt = max_dt_days.min(target_days - sim.time_days);
        sim.step(dt);
        assert!(
            sim.last_solver_warning.is_empty(),
            "gas_drive acceptance run emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }
}

/// Cumulative surface oil [Sm³] integrated over the recorded report schedule, the same way
/// the frontend's cumulative panel does it.
fn cumulative_oil_sc(sim: &ReservoirSimulator) -> f64 {
    let mut cumulative = 0.0;
    let mut previous_time_days = 0.0;
    for point in &sim.rate_history {
        cumulative += point.total_production_oil * (point.time - previous_time_days);
        previous_time_days = point.time;
    }
    cumulative
}

/// Stock-tank oil initially in place [Sm³] — the oil material-balance denominator.
fn stock_tank_oil_in_place_sm3(sim: &ReservoirSimulator) -> f64 {
    (0..sim.nx * sim.ny * sim.nz)
        .map(|id| {
            let bo = sim.get_b_o_cell(id, sim.pressure[id]).max(1e-9);
            sim.sat_oil[id] * sim.pore_volume_m3(id) / bo
        })
        .sum()
}

// ─── gas_drive: OPM Flow reference ───────────────────────────────────────────

/// `flow 2026.04` on `tools/opm_flow/opm_flow_tool/cases.py::GAS_DRIVE`, parsed from the run's
/// `.RSM` and also committed as `src/lib/catalog/opm-flow-results/gas_drive.json`.
/// Columns: time [days], FPR [bar], FOPR [Sm³/day], FOPT [Sm³], FGOR [Sm³/Sm³].
const OPM_GAS_DRIVE: [(f64, f64, f64, f64, f64); 11] = [
    (10.0, 171.3294, 40.60961, 679.7952, 486.9769),
    (20.0, 158.8346, 27.37219, 978.1079, 488.0770),
    (30.0, 150.0534, 21.92094, 1197.317, 489.2926),
    (50.0, 137.4439, 15.66580, 1537.213, 492.1335),
    (100.0, 119.8228, 8.056358, 2066.835, 500.5772),
    (150.0, 111.0780, 4.427388, 2351.238, 507.4564),
    (200.0, 106.3287, 2.518755, 2510.644, 511.9746),
    (300.0, 102.0964, 0.837407, 2654.952, 516.6122),
    (400.0, 100.7018, 0.280382, 2703.107, 518.5769),
    (500.0, 100.2358, 0.094192, 2719.259, 519.3077),
    (600.0, 100.0793, 0.031684, 2724.689, 519.5609),
];

/// Field average reservoir pressure.
const GAS_DRIVE_PRESSURE_TOLERANCE: f64 = 0.03;
/// Producing gas-oil ratio.
const GAS_DRIVE_GOR_TOLERANCE: f64 = 0.12;
/// Cumulative surface oil. This is the load-bearing oil criterion over the whole horizon:
/// the instantaneous rate decays below 1 Sm³/day, where a small absolute difference is a
/// large relative one, but the integral stays well conditioned.
const GAS_DRIVE_CUMULATIVE_OIL_TOLERANCE: f64 = 0.08;
/// Instantaneous producer oil rate, graded only while the reference rate is still
/// meaningfully large (see `GAS_DRIVE_MIN_GRADED_OIL_RATE_SC_DAY`).
const GAS_DRIVE_OIL_RATE_TOLERANCE: f64 = 0.10;
/// Below this reference oil rate [Sm³/day] the instantaneous rate is not graded; cumulative
/// oil carries the late-time comparison instead.
const GAS_DRIVE_MIN_GRADED_OIL_RATE_SC_DAY: f64 = 10.0;
/// Oil material-balance drift relative to stock-tank oil initially in place.
const GAS_DRIVE_OIL_MATERIAL_BALANCE_TOLERANCE: f64 = 0.01;
/// Gas material-balance drift relative to total gas initially in place (free + dissolved).
/// The case has no injection, so gas in place is all the gas the case ever handles.
const GAS_DRIVE_GAS_MATERIAL_BALANCE_TOLERANCE: f64 = 0.01;

/// Solution gas drive graded against the OPM Flow reference solution.
///
/// This is the comparative-solution anchor for the depletion side of three-phase, the way
/// SPE1 is for the injection side. It runs the whole 600-day scenario horizon — cheap enough
/// (~1 s debug) to stay a default gate rather than an `#[ignore]`d replay.
#[test]
fn three_phase_gas_drive_matches_opm_flow_reference() {
    let mut sim = make_gas_drive_acceptance_sim();
    let stoiip_sm3 = stock_tank_oil_in_place_sm3(&sim);
    let gas_in_place_sm3 = total_gas_inventory_sc_all_cells(&sim);

    for (t_days, ref_pressure, ref_oil_rate, ref_cumulative_oil, ref_gor) in OPM_GAS_DRIVE {
        step_to(&mut sim, t_days, 10.0);
        let point = sim.rate_history.last().expect("rate history");

        let pressure_error = (point.avg_reservoir_pressure - ref_pressure).abs() / ref_pressure;
        assert!(
            pressure_error <= GAS_DRIVE_PRESSURE_TOLERANCE,
            "gas_drive average reservoir pressure outside acceptance band at t={t_days}: got {:.3} bar, OPM {:.3} bar, error {:.2}% > {:.2}%",
            point.avg_reservoir_pressure,
            ref_pressure,
            pressure_error * 100.0,
            GAS_DRIVE_PRESSURE_TOLERANCE * 100.0
        );

        let gor_error = (point.producing_gor - ref_gor).abs() / ref_gor;
        assert!(
            gor_error <= GAS_DRIVE_GOR_TOLERANCE,
            "gas_drive producing GOR outside acceptance band at t={t_days}: got {:.3} Sm3/Sm3, OPM {:.3} Sm3/Sm3, error {:.2}% > {:.2}%",
            point.producing_gor,
            ref_gor,
            gor_error * 100.0,
            GAS_DRIVE_GOR_TOLERANCE * 100.0
        );

        let cumulative_oil = cumulative_oil_sc(&sim);
        let cumulative_oil_error = (cumulative_oil - ref_cumulative_oil).abs() / ref_cumulative_oil;
        assert!(
            cumulative_oil_error <= GAS_DRIVE_CUMULATIVE_OIL_TOLERANCE,
            "gas_drive cumulative oil outside acceptance band at t={t_days}: got {:.3} Sm3, OPM {:.3} Sm3, error {:.2}% > {:.2}%",
            cumulative_oil,
            ref_cumulative_oil,
            cumulative_oil_error * 100.0,
            GAS_DRIVE_CUMULATIVE_OIL_TOLERANCE * 100.0
        );

        if ref_oil_rate >= GAS_DRIVE_MIN_GRADED_OIL_RATE_SC_DAY {
            let oil_rate_error = (point.total_production_oil - ref_oil_rate).abs() / ref_oil_rate;
            assert!(
                oil_rate_error <= GAS_DRIVE_OIL_RATE_TOLERANCE,
                "gas_drive producer oil rate outside acceptance band at t={t_days}: got {:.4} Sm3/d, OPM {:.4} Sm3/d, error {:.2}% > {:.2}%",
                point.total_production_oil,
                ref_oil_rate,
                oil_rate_error * 100.0,
                GAS_DRIVE_OIL_RATE_TOLERANCE * 100.0
            );
        }

        let oil_drift = point.material_balance_error_oil_m3.abs() / stoiip_sm3;
        assert!(
            oil_drift <= GAS_DRIVE_OIL_MATERIAL_BALANCE_TOLERANCE,
            "gas_drive oil material-balance drift too large at t={t_days}: {:.3} Sm3 = {:.4}% of STOIIP ({:.1} Sm3) > {:.3}%",
            point.material_balance_error_oil_m3,
            oil_drift * 100.0,
            stoiip_sm3,
            GAS_DRIVE_OIL_MATERIAL_BALANCE_TOLERANCE * 100.0
        );

        let gas_drift = point.material_balance_error_gas_m3.abs() / gas_in_place_sm3;
        assert!(
            gas_drift <= GAS_DRIVE_GAS_MATERIAL_BALANCE_TOLERANCE,
            "gas_drive gas material-balance drift too large at t={t_days}: {:.3} Sm3 = {:.4}% of gas in place ({:.1} Sm3) > {:.3}%",
            point.material_balance_error_gas_m3,
            gas_drift * 100.0,
            gas_in_place_sm3,
            GAS_DRIVE_GAS_MATERIAL_BALANCE_TOLERANCE * 100.0
        );
    }
}

/// Characterisation replay: prints the errors actually measured against the OPM reference at
/// every checkpoint, plus the measured breakthrough times, so the baseline table in
/// `docs/THREE_PHASE_VALIDATION.md` can be regenerated verbatim. Asserts nothing beyond what
/// the acceptance gate above already asserts.
///
/// ```text
/// cargo test --manifest-path src/lib/ressim/Cargo.toml \
///   three_phase_acceptance_error_replay -- --ignored --nocapture
/// ```
#[test]
#[ignore = "characterization replay: prints measured three-phase acceptance errors (see docs/THREE_PHASE_VALIDATION.md)"]
fn three_phase_acceptance_error_replay() {
    let mut sim = make_gas_drive_acceptance_sim();
    let stoiip_sm3 = stock_tank_oil_in_place_sm3(&sim);
    let gas_in_place_sm3 = total_gas_inventory_sc_all_cells(&sim);

    for (t_days, ref_pressure, ref_oil_rate, ref_cumulative_oil, ref_gor) in OPM_GAS_DRIVE {
        step_to(&mut sim, t_days, 10.0);
        let point = sim.rate_history.last().expect("rate history");
        let oil_rate_error = if ref_oil_rate >= GAS_DRIVE_MIN_GRADED_OIL_RATE_SC_DAY {
            format!(
                "{:6.3}%",
                (point.total_production_oil - ref_oil_rate).abs() / ref_oil_rate * 100.0
            )
        } else {
            "     --".to_string()
        };
        println!(
            "t={:6.1} pressure_err={:6.3}% oil_rate_err={} cum_oil_err={:6.3}% gor_err={:6.3}% mb_oil={:7.4}% mb_gas={:7.4}%",
            t_days,
            (point.avg_reservoir_pressure - ref_pressure).abs() / ref_pressure * 100.0,
            oil_rate_error,
            (cumulative_oil_sc(&sim) - ref_cumulative_oil).abs() / ref_cumulative_oil * 100.0,
            (point.producing_gor - ref_gor).abs() / ref_gor * 100.0,
            point.material_balance_error_oil_m3.abs() / stoiip_sm3 * 100.0,
            point.material_balance_error_gas_m3.abs() / gas_in_place_sm3 * 100.0,
        );
    }

    println!(
        "gas-flood breakthrough: dt=1.0 -> {:?} days, dt=0.5 -> {:?} days",
        gas_breakthrough_time_days(20, 1.0, 40.0),
        gas_breakthrough_time_days(20, 0.5, 40.0),
    );
}

/// The mechanism the case is named after: pressure must fall below the bubble point, the
/// oil's dissolved gas must come out of solution, free gas must accumulate, and the producing
/// GOR must rise monotonically as a result.
///
/// Without this, the OPM comparison alone could be satisfied by a model that happened to
/// track the pressure and rate curves without actually liberating gas.
#[test]
fn three_phase_gas_drive_liberates_solution_gas_as_pressure_falls() {
    let mut sim = make_gas_drive_acceptance_sim();
    let initial_avg_rs = sim.rs.iter().copied().sum::<f64>() / sim.rs.len() as f64;
    let initial_avg_sg = sim.sat_gas.iter().copied().sum::<f64>() / sim.sat_gas.len() as f64;

    let mut previous_gor = f64::NEG_INFINITY;
    let mut previous_avg_sg = initial_avg_sg;
    for t_days in [50.0, 100.0, 200.0, 400.0, 600.0] {
        step_to(&mut sim, t_days, 10.0);
        let point = sim.rate_history.last().expect("rate history");

        assert!(
            point.avg_reservoir_pressure < GAS_DRIVE_BUBBLE_POINT_BAR,
            "gas_drive should be below the bubble point at t={t_days}: {:.3} bar",
            point.avg_reservoir_pressure
        );
        assert!(
            point.producing_gor > previous_gor,
            "gas_drive producing GOR should rise as pressure falls, but went {:.3} -> {:.3} at t={t_days}",
            previous_gor,
            point.producing_gor
        );
        assert!(
            point.avg_gas_saturation > previous_avg_sg,
            "gas_drive free-gas saturation should keep building, but went {:.5} -> {:.5} at t={t_days}",
            previous_avg_sg,
            point.avg_gas_saturation
        );
        previous_gor = point.producing_gor;
        previous_avg_sg = point.avg_gas_saturation;
    }

    let final_avg_rs = sim.rs.iter().copied().sum::<f64>() / sim.rs.len() as f64;
    assert!(
        final_avg_rs < initial_avg_rs,
        "gas_drive should strip dissolved gas out of the oil: Rs went {initial_avg_rs:.4} -> {final_avg_rs:.4} Sm3/Sm3"
    );
    assert!(
        previous_avg_sg > initial_avg_sg,
        "gas_drive should end with more free gas than it started with: Sg {initial_avg_sg:.5} -> {previous_avg_sg:.5}"
    );
}

// ─── gas flood: front behavior ───────────────────────────────────────────────

/// Producer-cell gas saturation that counts as gas breakthrough.
const BREAKTHROUGH_SG_THRESHOLD: f64 = 1e-3;
/// Acceptance band for breakthrough time [days] on the 20-cell 1D gas flood.
const BREAKTHROUGH_MIN_DAYS: f64 = 2.0;
const BREAKTHROUGH_MAX_DAYS: f64 = 8.0;
/// Breakthrough time must not move more than this [days] when the report step is halved.
const BREAKTHROUGH_REFINEMENT_TOLERANCE_DAYS: f64 = 1.5;

/// First time [days] the producer cell's gas saturation crosses the breakthrough threshold,
/// stepping at `dt_days` for at most `max_days`.
fn gas_breakthrough_time_days(nx: usize, dt_days: f64, max_days: f64) -> Option<f64> {
    let mut sim = make_3phase_gas_injection_sim(nx, true);
    let producer_id = sim.idx(nx - 1, 0, 0);
    while sim.time_days < max_days {
        sim.step(dt_days);
        assert!(
            sim.last_solver_warning.is_empty(),
            "gas flood emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
        if sim.sat_gas[producer_id] > BREAKTHROUGH_SG_THRESHOLD {
            return Some(sim.time_days);
        }
    }
    None
}

/// Gas breakthrough must happen, must happen inside the acceptance band, and must not move
/// materially when the report step is halved.
///
/// The existing refinement test only checks that *ordering* is preserved; this one pins the
/// timing itself, which is what a user reads off the GOR chart.
#[test]
fn three_phase_gas_flood_breakthrough_time_is_within_acceptance_band() {
    let coarse = gas_breakthrough_time_days(20, 1.0, 40.0)
        .expect("1D gas flood should reach producer gas breakthrough within 40 days at dt=1.0");
    let fine = gas_breakthrough_time_days(20, 0.5, 40.0)
        .expect("1D gas flood should reach producer gas breakthrough within 40 days at dt=0.5");

    assert!(
        (BREAKTHROUGH_MIN_DAYS..=BREAKTHROUGH_MAX_DAYS).contains(&coarse),
        "1D gas-flood breakthrough time outside acceptance band: {coarse} days, band [{BREAKTHROUGH_MIN_DAYS}, {BREAKTHROUGH_MAX_DAYS}]"
    );
    assert!(
        (BREAKTHROUGH_MIN_DAYS..=BREAKTHROUGH_MAX_DAYS).contains(&fine),
        "1D gas-flood breakthrough time at the refined step outside acceptance band: {fine} days, band [{BREAKTHROUGH_MIN_DAYS}, {BREAKTHROUGH_MAX_DAYS}]"
    );
    assert!(
        (coarse - fine).abs() <= BREAKTHROUGH_REFINEMENT_TOLERANCE_DAYS,
        "1D gas-flood breakthrough time moved under timestep refinement: dt=1.0 gave {coarse} days, dt=0.5 gave {fine} days, tolerance {BREAKTHROUGH_REFINEMENT_TOLERANCE_DAYS} days"
    );
}

/// Gas-saturation evolution, as a shape rather than as bounds: the flood must build a front
/// that decreases monotonically from injector to producer, and every cell's gas saturation
/// must increase monotonically in time while gas keeps being injected.
#[test]
fn three_phase_gas_flood_saturation_front_is_monotone_and_advances() {
    const NX: usize = 20;
    // Round-off headroom on the saturation comparisons — the transport update clamps and
    // renormalises, so exact monotonicity is not expected at the 1e-16 level.
    const SATURATION_EPSILON: f64 = 1e-9;

    let mut sim = make_3phase_gas_injection_sim(NX, true);
    let mut previous_sg = sim.sat_gas.clone();
    let mut front_position = 0usize;

    for _ in 0..30 {
        sim.step(1.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "gas flood emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );

        for i in 0..NX - 1 {
            assert!(
                sim.sat_gas[i] + SATURATION_EPSILON >= sim.sat_gas[i + 1],
                "gas-flood saturation profile is not monotone at t={}: Sg[{}]={:.6} < Sg[{}]={:.6}",
                sim.time_days,
                i,
                sim.sat_gas[i],
                i + 1,
                sim.sat_gas[i + 1]
            );
        }

        for i in 0..NX {
            assert!(
                sim.sat_gas[i] + SATURATION_EPSILON >= previous_sg[i],
                "gas-flood saturation went backwards in cell {} at t={}: {:.6} -> {:.6}",
                i,
                sim.time_days,
                previous_sg[i],
                sim.sat_gas[i]
            );
        }

        let reached = sim
            .sat_gas
            .iter()
            .rposition(|&sg| sg > BREAKTHROUGH_SG_THRESHOLD)
            .unwrap_or(0);
        assert!(
            reached >= front_position,
            "gas-flood front receded at t={}: furthest invaded cell went {} -> {}",
            sim.time_days,
            front_position,
            reached
        );
        front_position = reached;
        previous_sg.copy_from_slice(&sim.sat_gas);
    }

    assert_eq!(
        front_position,
        NX - 1,
        "gas-flood front should have reached the producer cell within 30 days; furthest invaded cell was {front_position}"
    );
}

/// Explicit closure for all three phases in a gas flood, oil included.
///
/// Oil is the residual *saturation* in the transport update (S_o = 1 − S_w − S_g), but its
/// material balance is not a residual: `material_balance_error_oil_m3` compares reported
/// surface oil production against the actual stock-tank oil inventory change. Asserting it
/// here is what makes oil closure a graded quantity in three-phase rather than an assumption
/// inherited from the saturation constraint.
#[test]
fn three_phase_gas_flood_phase_closure_holds_for_all_three_phases() {
    const NX: usize = 20;
    /// Per-phase drift relative to that phase's initial inventory.
    const PHASE_CLOSURE_TOLERANCE: f64 = 0.01;

    let mut sim = make_3phase_gas_injection_sim(NX, true);
    let stoiip_sm3 = stock_tank_oil_in_place_sm3(&sim);
    let initial_water_m3: f64 = (0..NX)
        .map(|i| sim.sat_water[i] * sim.pore_volume_m3(i))
        .sum();

    let mut injected_gas_sc = 0.0;
    let mut previous_time_days = 0.0;
    for _ in 0..30 {
        sim.step(1.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "gas flood emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );

        let point = sim.rate_history.last().expect("rate history");
        injected_gas_sc += point.total_injection.max(0.0) * (point.time - previous_time_days);
        previous_time_days = point.time;

        for (phase, drift, denominator) in [
            ("oil", point.material_balance_error_oil_m3.abs(), stoiip_sm3),
            (
                "water",
                point.material_balance_error_m3.abs(),
                initial_water_m3,
            ),
            (
                "gas",
                point.material_balance_error_gas_m3.abs(),
                total_gas_inventory_sc_all_cells(&sim) + injected_gas_sc,
            ),
        ] {
            let relative = drift / denominator.max(1e-9);
            assert!(
                relative <= PHASE_CLOSURE_TOLERANCE,
                "gas-flood {phase} material-balance drift too large at t={}: {drift:.4} = {:.4}% of {denominator:.1} > {:.3}%",
                sim.time_days,
                relative * 100.0,
                PHASE_CLOSURE_TOLERANCE * 100.0
            );
        }
    }
}
