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
use crate::tests::bench_record::{self, Metric, Worst};
use crate::tests::physics::fixtures::{
    make_3phase_gas_injection_sim, total_gas_inventory_sc_all_cells,
};

// ─── gas_drive: scenario mirror ──────────────────────────────────────────────

/// Bubble point of the `gas_drive` PVT table [bar]. Equal to the initial pressure, so the
/// reservoir starts saturated and drawdown liberates gas immediately.
const GAS_DRIVE_BUBBLE_POINT_BAR: f64 = 200.0;
/// Solution GOR at the bubble point [Sm³/Sm³].
const GAS_DRIVE_INITIAL_RS: f64 = 131.58664;
const GAS_DRIVE_PRODUCER_BHP_BAR: f64 = 100.0;
const GAS_DRIVE_INITIAL_SW: f64 = 0.2;
const GAS_DRIVE_INITIAL_SG: f64 = 0.08;

/// The `gas_drive` scenario's PVT table, i.e. the output of
/// `generateBlackOilTable(35 API, 0.75 gas gravity, 80 C, Pb = 200 bar, Pmax = 300 bar,
/// 20 points, c_o = 1e-5/bar)`. The same 20 rows are emitted verbatim into the OPM deck's
/// PVTO/PVDG, so engine and reference read identical fluid properties. Regenerated for #60, when
/// `standingRs` got its API/temperature exponent sign fixed (Rs_b 28.2 -> 131.6 m3/m3).
fn gas_drive_pvt_rows() -> Vec<PvtRow> {
    const ROWS: [(f64, f64, f64, f64, f64, f64); 20] = [
        (1.0000, 0.22227, 1.05404, 2.26301, 1.239768, 0.01254),
        (15.7895, 6.17594, 1.06753, 1.85160, 0.076466, 0.01271),
        (31.5789, 14.23606, 1.08634, 1.50971, 0.037254, 0.01299),
        (47.3684, 23.20320, 1.10791, 1.26949, 0.024222, 0.01337),
        (63.1579, 32.81530, 1.13170, 1.09589, 0.017742, 0.01382),
        (78.9474, 42.93737, 1.15742, 0.96575, 0.013889, 0.01436),
        (94.7368, 53.48532, 1.18489, 0.86495, 0.011355, 0.01498),
        (110.5263, 64.40111, 1.21397, 0.78468, 0.009579, 0.01568),
        (126.3158, 75.64203, 1.24455, 0.71928, 0.008280, 0.01646),
        (142.1053, 87.17515, 1.27653, 0.66497, 0.007300, 0.01731),
        (157.8947, 98.97425, 1.30986, 0.61913, 0.006546, 0.01821),
        (173.6842, 111.01788, 1.34445, 0.57991, 0.005954, 0.01916),
        (189.4737, 123.28815, 1.38026, 0.54596, 0.005483, 0.02014),
        (200.0000, 131.58664, 1.40479, 0.52574, 0.005220, 0.02081),
        (221.0526, 131.58664, 1.40449, 0.54083, 0.004791, 0.02214),
        (236.8421, 131.58664, 1.40427, 0.55316, 0.004534, 0.02313),
        (252.6316, 131.58664, 1.40405, 0.56629, 0.004318, 0.02412),
        (268.4211, 131.58664, 1.40383, 0.58017, 0.004135, 0.02508),
        (284.2105, 131.58664, 1.40361, 0.59475, 0.003978, 0.02603),
        (300.0000, 131.58664, 1.40338, 0.61001, 0.003842, 0.02695),
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
pub(super) fn make_gas_drive_acceptance_sim() -> ReservoirSimulator {
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
/// `.RSM` and also committed as `src/lib/catalog/opm-flow-results/gas_drive.json`. Since #55 that
/// deck is `opm/reference-decks/small-direct/gas-drive-20`, written by `opm_small_direct.rs` from
/// [`make_gas_drive_acceptance_sim`] itself; the hand-written deck before it sampled SGOF at ten
/// nodes and put the reference 4-6 % off this model. Re-run when that deck's SGOF gained nodes
/// just above S_gc (#55), which moved these values by ≤ 0.1 %.
/// Columns: time [days], FPR [bar], FOPR [Sm³/day], FOPT [Sm³], FGOR [Sm³/Sm³].
const OPM_GAS_DRIVE: [(f64, f64, f64, f64, f64); 11] = [
    (10.0, 171.4955, 47.63953, 829.5626, 1045.650),
    (20.0, 158.9385, 32.04709, 1178.726, 1053.351),
    (30.0, 150.0524, 25.85907, 1437.316, 1057.697),
    (50.0, 137.2212, 18.89210, 1844.752, 1069.504),
    (100.0, 118.9864, 9.811957, 2491.692, 1121.498),
    (150.0, 109.9570, 5.105173, 2828.874, 1171.502),
    (200.0, 105.2643, 2.668444, 3004.686, 1204.136),
    (300.0, 101.4909, 0.7450863, 3145.649, 1233.640),
    (400.0, 100.4269, 0.2123299, 3185.442, 1242.542),
    (500.0, 100.1226, 0.06089768, 3196.822, 1245.124),
    (600.0, 100.0352, 0.01749878, 3200.090, 1245.869),
];

// Tightened in #55, when the reference moved from the hand-written deck to one generated from
// this simulator. Against the old reference the worst errors were 1.6 / 6.1 / 4.3 / 4.6 %
// (pressure / GOR / cumulative oil / oil rate), almost all of it the old deck's coarse SGOF.
// Measured against the new one (flow 2026.04): 0.29 / 0.26 / 1.38 / 3.91 %; re-run on the
// corrected Standing fluid (#60, Rs_b 28 -> 132 m3/m3): 0.28 / 0.56 / 1.37 / 3.92 %. The oil-rate and
// cumulative-oil worsts both sit at 20 d, in the steep first transient where the two simulators'
// time steps differ; each band keeps 2-4x headroom over its measured error.

/// Field average reservoir pressure. Measured 0.28 % (20 d).
const GAS_DRIVE_PRESSURE_TOLERANCE: f64 = 0.01;
/// Producing gas-oil ratio. Measured 0.56 % (50 d).
const GAS_DRIVE_GOR_TOLERANCE: f64 = 0.01;
/// Cumulative surface oil. This is the load-bearing oil criterion over the whole horizon:
/// the instantaneous rate decays below 1 Sm³/day, where a small absolute difference is a
/// large relative one, but the integral stays well conditioned. Measured 1.37 % (20 d),
/// 0.32 % by 600 d.
const GAS_DRIVE_CUMULATIVE_OIL_TOLERANCE: f64 = 0.03;
/// Instantaneous producer oil rate, graded only while the reference rate is still
/// meaningfully large (see `GAS_DRIVE_MIN_GRADED_OIL_RATE_SC_DAY`). Measured 3.92 % (20 d).
const GAS_DRIVE_OIL_RATE_TOLERANCE: f64 = 0.08;
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

    // Signed (ResSim minus Flow), so a one-sided bias shows as one.
    let mut worst = [Worst::new(); 6];
    // The checkpoint series, for the aggregator to compare against a second simulator (#54).
    let mut series: [Vec<f64>; 4] = Default::default();
    for (t_days, ref_pressure, ref_oil_rate, ref_cumulative_oil, ref_gor) in OPM_GAS_DRIVE {
        step_to(&mut sim, t_days, 10.0);
        let point = sim.rate_history.last().expect("rate history");
        series[0].push(point.avg_reservoir_pressure);
        series[1].push(point.total_production_oil);
        series[2].push(cumulative_oil_sc(&sim));
        series[3].push(point.producing_gor);
        worst[0].update(
            (point.avg_reservoir_pressure - ref_pressure) / ref_pressure,
            t_days,
        );
        worst[1].update((point.producing_gor - ref_gor) / ref_gor, t_days);
        worst[2].update(
            (cumulative_oil_sc(&sim) - ref_cumulative_oil) / ref_cumulative_oil,
            t_days,
        );
        if ref_oil_rate >= GAS_DRIVE_MIN_GRADED_OIL_RATE_SC_DAY {
            worst[3].update(
                (point.total_production_oil - ref_oil_rate) / ref_oil_rate,
                t_days,
            );
        }
        worst[4].update(point.material_balance_error_oil_m3 / stoiip_sm3, t_days);
        worst[5].update(
            point.material_balance_error_gas_m3 / gas_in_place_sm3,
            t_days,
        );
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

    for ((metric, band), found) in [
        ("pressure_rel_err", GAS_DRIVE_PRESSURE_TOLERANCE),
        ("gor_rel_err", GAS_DRIVE_GOR_TOLERANCE),
        ("cum_oil_rel_err", GAS_DRIVE_CUMULATIVE_OIL_TOLERANCE),
        ("oil_rate_rel_err", GAS_DRIVE_OIL_RATE_TOLERANCE),
        ("mb_drift_oil", GAS_DRIVE_OIL_MATERIAL_BALANCE_TOLERANCE),
        ("mb_drift_gas", GAS_DRIVE_GAS_MATERIAL_BALANCE_TOLERANCE),
    ]
    .into_iter()
    .zip(worst)
    {
        bench_record::metric(Metric {
            section: "three_phase",
            case: "gas_drive",
            metric,
            value: found.value,
            band: Some(band),
            unit: "frac",
            reference: "OPM Flow, hand-mapped deck",
            // The deck shares PVT, SCAL, grid and wells with the engine setup by construction
            // (`docs/THREE_PHASE_VALIDATION.md`), so a gap is a finding, not resolution.
            same_model: true,
            at: &found.at(),
        });
    }

    let times: Vec<f64> = OPM_GAS_DRIVE.iter().map(|row| row.0).collect();
    let flow: [Vec<f64>; 4] = [
        OPM_GAS_DRIVE.iter().map(|row| row.1).collect(),
        OPM_GAS_DRIVE.iter().map(|row| row.2).collect(),
        OPM_GAS_DRIVE.iter().map(|row| row.3).collect(),
        OPM_GAS_DRIVE.iter().map(|row| row.4).collect(),
    ];
    for (index, quantity) in ["FPR", "FOPR", "FOPT", "FGOR"].into_iter().enumerate() {
        bench_record::series(
            "three_phase",
            "gas_drive",
            "ressim",
            quantity,
            &times,
            &series[index],
        );
        bench_record::series(
            "three_phase",
            "gas_drive",
            "flow",
            quantity,
            &times,
            &flow[index],
        );
    }

    let breakthrough_coarse = gas_breakthrough_time_days(20, 1.0, 40.0);
    let breakthrough_fine = gas_breakthrough_time_days(20, 0.5, 40.0);
    println!(
        "gas-flood breakthrough: dt=1.0 -> {:?} days, dt=0.5 -> {:?} days",
        breakthrough_coarse, breakthrough_fine,
    );
    for (case, found) in [
        ("dt=1.0", breakthrough_coarse),
        ("dt=0.5", breakthrough_fine),
    ] {
        bench_record::metric(Metric {
            section: "three_phase",
            case: &format!("gas_flood {case}"),
            metric: "breakthrough_days",
            value: found.unwrap_or(f64::NAN),
            band: None,
            unit: "d",
            reference: "band 2-8 d (physics::gas_flood)",
            same_model: false,
            at: "",
        });
    }
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
        // `step` records one rate point per accepted substep, so every new point is integrated;
        // reading only the last would bill the whole step at its final substep's rate (#53).
        let first_new_point = sim.rate_history.len();
        sim.step(1.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "gas flood emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );

        for point in &sim.rate_history[first_new_point..] {
            injected_gas_sc += point.total_injection.max(0.0) * (point.time - previous_time_days);
            previous_time_days = point.time;
        }
        let point = sim.rate_history.last().expect("rate history");

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

// ─── gas_injection: OPM Flow twin ────────────────────────────────────────────

/// `flow 2026.04` on `opm/reference-decks/small-direct/go-1d-50`, the `gas_injection` scenario's
/// base case written from `opm_small_direct::gas_injection_1d()` (#12): (t [d], FOPT, FGPT, FGIT)
/// [Sm³]. Gas breaks through between 170 and 180 d in both simulators. Re-baselined for #42, when
/// table-less gas became compressible and the deck's PVDG followed, and for #55, when the deck's
/// SGOF gained nodes just above S_gc (≤ 0.03 % here).
const OPM_GAS_INJECTION: [(f64, f64, f64, f64); 3] = [
    (100.0, 6167.628, 0.000, 6226.662),
    (200.0, 15379.979, 3657.553, 19077.826),
    (300.0, 20331.672, 28986.195, 49359.785),
];
/// Oil produced and gas injected. Measured ≤ 0.046 % at every 20-day checkpoint (2026-09-24).
const GAS_INJECTION_CUMULATIVE_TOLERANCE: f64 = 0.002;
/// Gas produced, which starts at breakthrough and so is most sensitive to front timing.
/// Measured 0.22 % at 200 d, the worst checkpoint (re-run for #55; 0.29 % before).
const GAS_INJECTION_GAS_PRODUCED_TOLERANCE: f64 = 0.015;

/// #12: `gas_injection` against an independent simulator, not only its gas-oil fractional-flow
/// solution. The Flow deck is written from the same simulator object this test runs, so the two
/// read identical tables, including table-less gas's `Bg = exp(−c_g·(p − p_ref))` (#42).
#[test]
fn three_phase_gas_injection_matches_opm_flow_twin() {
    let mut sim = super::opm_small_direct::gas_injection_1d();
    let (mut oil, mut gas_produced, mut gas_injected) = (0.0, 0.0, 0.0);
    let mut previous_time = 0.0;
    let mut seen = 0;
    let (mut worst_oil, mut worst_injected, mut worst_produced) =
        (Worst::new(), Worst::new(), Worst::new());
    for (t_days, flow_oil, flow_gas_produced, flow_gas_injected) in OPM_GAS_INJECTION {
        while sim.time_days < t_days - 1e-9 {
            sim.step(2.0);
            assert!(
                sim.last_solver_warning.is_empty(),
                "gas_injection twin warned at t={}: {}",
                sim.time_days,
                sim.last_solver_warning
            );
        }
        for point in &sim.rate_history[seen..] {
            let dt = point.time - previous_time;
            oil += point.total_production_oil * dt;
            gas_produced += point.total_production_gas * dt;
            gas_injected += point.total_injection * dt;
            previous_time = point.time;
        }
        seen = sim.rate_history.len();

        let rel = |ours: f64, flow: f64| (ours - flow).abs() / flow;
        worst_oil.update((oil - flow_oil) / flow_oil, t_days);
        worst_injected.update(
            (gas_injected - flow_gas_injected) / flow_gas_injected,
            t_days,
        );
        if flow_gas_produced > 0.0 {
            worst_produced.update(
                (gas_produced - flow_gas_produced) / flow_gas_produced,
                t_days,
            );
        }
        assert!(
            rel(oil, flow_oil) <= GAS_INJECTION_CUMULATIVE_TOLERANCE,
            "t={t_days}: FOPT {oil:.3} vs Flow {flow_oil:.3}"
        );
        assert!(
            rel(gas_injected, flow_gas_injected) <= GAS_INJECTION_CUMULATIVE_TOLERANCE,
            "t={t_days}: FGIT {gas_injected:.3} vs Flow {flow_gas_injected:.3}"
        );
        if flow_gas_produced > 0.0 {
            assert!(
                rel(gas_produced, flow_gas_produced) <= GAS_INJECTION_GAS_PRODUCED_TOLERANCE,
                "t={t_days}: FGPT {gas_produced:.3} vs Flow {flow_gas_produced:.3}"
            );
        } else {
            assert!(
                gas_produced < 1.0,
                "t={t_days}: gas produced before Flow's breakthrough: {gas_produced:.3}"
            );
        }
    }

    for (metric, found, band) in [
        (
            "cum_oil_rel_err",
            worst_oil,
            GAS_INJECTION_CUMULATIVE_TOLERANCE,
        ),
        (
            "cum_gas_injected_rel_err",
            worst_injected,
            GAS_INJECTION_CUMULATIVE_TOLERANCE,
        ),
        (
            "cum_gas_produced_rel_err",
            worst_produced,
            GAS_INJECTION_GAS_PRODUCED_TOLERANCE,
        ),
    ] {
        bench_record::metric(Metric {
            section: "three_phase",
            case: "gas_injection (go-1d-50)",
            metric,
            value: found.value,
            band: Some(band),
            unit: "frac",
            reference: "OPM Flow, generated deck",
            same_model: true,
            at: &found.at(),
        });
    }
}

/// #43: pore volumes injected is a reservoir volume over the pore volume, so the rate history
/// has to carry injection at reservoir conditions and not only at the surface. With gas on a
/// black-oil table the two differ by Bg ≈ 0.004, not the ≈ 1 of table-less gas, so a frontend
/// that built PVI from the surface series was ~250x off here. This pins both reservoir figures
/// on both solvers: `total_injection_reservoir` at the injector's own conditions, and
/// `total_injection_resv` at the average reservoir pressure, which PVI is built from.
#[test]
fn three_phase_gas_injector_reports_reservoir_injection_at_its_bg() {
    for fim in [true, false] {
        let mut sim = super::opm_small_direct::gas_injection_1d();
        sim.set_fim_enabled(fim);
        sim.apply_pvt_table(gas_drive_pvt_rows()).unwrap();
        // Inside the table (1-300 bar), above its 200 bar bubble point.
        for well in &mut sim.wells {
            well.bhp = if well.injector { 290.0 } else { 210.0 };
        }
        sim.set_well_bhp_limits(210.0, 290.0).unwrap();
        let injector_cell = sim
            .wells
            .iter()
            .find(|well| well.injector)
            .map(|well| sim.idx(well.i, well.j, well.k))
            .unwrap();

        for _ in 0..5 {
            sim.step(1.0);
            let point = sim.rate_history.last().unwrap();
            assert!(point.total_injection > 0.0, "fim={fim}: no gas injected");
            let ratio = point.total_injection_reservoir / point.total_injection;
            // The surface conversion is taken between the cell and the wellbore, so Bg of
            // either bounds it.
            let bg_cell = sim.get_b_g(sim.pressure[injector_cell]);
            let bg_well = sim.get_b_g(290.0);
            let (lo, hi) = (bg_cell.min(bg_well), bg_cell.max(bg_well));
            assert!(
                ratio > 0.98 * lo && ratio < 1.02 * hi,
                "fim={fim} t={}: reservoir/surface injection {ratio:.6} outside Bg [{lo:.6}, {hi:.6}]",
                point.time
            );
            assert!(
                ratio < 0.01,
                "fim={fim}: Bg should be far from 1 here, got {ratio}"
            );
            // PVI's numerator: the same surface rate at Bg of the average pressure, the RESV
            // convention Flow's FVIT uses.
            let resv = point.total_injection_resv.expect("RESV injection reported");
            let bg_avg = sim.get_b_g(point.avg_reservoir_pressure);
            assert!(
                (resv - point.total_injection * bg_avg).abs() <= 1e-12 * resv.abs(),
                "fim={fim}: RESV injection {resv} is not surface {} x Bg(p_avg) {bg_avg}",
                point.total_injection
            );
        }
    }
}
