//! Small-system OPM cross-check (FIM-DIRECT-001 follow-up).
//!
//! Every case here stays under the 512-row forced-direct threshold, so it exercises the small
//! direct-LU route that the existing OPM decks (all ~900+ rows) never reach. One definition drives
//! both sides: [`build`] configures ResSim, and [`deck`] writes the Flow deck from that *same*
//! simulator, evaluating relative permeability and PVT through ResSim's own functions onto dense
//! tables. A mapping written out by hand drifts; this one cannot disagree about the inputs
//! without disagreeing with the engine.
//!
//! Both halves are `#[ignore]`d drivers, not gates:
//!
//! ```bash
//! # write the decks (committed under opm/reference-decks/small-direct/)
//! OPM_SMALL_DECK_DIR=opm/reference-decks/small-direct cargo test --release \
//!   --manifest-path src/lib/ressim/Cargo.toml opm_small_direct_write_decks -- --ignored
//! # run ResSim: FIM once per small-system LU (OPM_SMALL_BACKEND), and IMPES
//! OPM_SMALL_OUT=/tmp/small-direct OPM_SMALL_SOLVER=fim OPM_SMALL_BACKEND=sparse cargo test --release \
//!   --manifest-path src/lib/ressim/Cargo.toml opm_small_direct_run_ressim -- --ignored --nocapture
//! OPM_SMALL_OUT=/tmp/small-direct OPM_SMALL_SOLVER=impes cargo test --release \
//!   --manifest-path src/lib/ressim/Cargo.toml opm_small_direct_run_ressim -- --ignored --nocapture
//! ```
//!
//! `tools/opm_flow/compare_small_direct.py` runs Flow on the decks and compares every run against
//! it and against each other. `scripts/validate-cross-solver.sh` does all of the above and checks
//! the result against the committed scorecard.

use std::fmt::Write as _;
use std::time::Instant;

use super::*;
use crate::pvt::{PvtRow, PvtTable};

/// Flow is always run with `--enable-gravity=false`; every case here is gravity-off.
struct Case {
    key: &'static str,
    note: &'static str,
    report_steps: usize,
    report_dt_days: f64,
}

const CASES: [Case; 8] = [
    Case {
        key: "ow-1d-96",
        note: "wf_bl1d geometry: 96x1x1 waterflood, BHP injector/producer, M~2",
        report_steps: 120,
        report_dt_days: 0.25,
    },
    Case {
        key: "ow-1d-50-adverse",
        note: "the parity matrix's adverse-mobility FIM case (mu_o = 20) over 3 days",
        report_steps: 12,
        report_dt_days: 0.25,
    },
    Case {
        key: "ow-2d-12x12",
        note: "12x12 heterogeneous quarter five-spot; the 9,021-substep case before FIM-DIRECT-001",
        report_steps: 40,
        report_dt_days: 0.25,
    },
    Case {
        key: "bo-1d-10",
        note: "physics_depletion_grid_convergence_fim column at nx=10: depletion through the bubble point",
        report_steps: 20,
        report_dt_days: 5.0,
    },
    Case {
        key: "bo-1d-40",
        note: "the same column at nx=40",
        report_steps: 20,
        report_dt_days: 5.0,
    },
    Case {
        key: "go-1d-50",
        note: "gas_injection scenario base case: dead oil displaced by gas in a 50-cell column, both wells on BHP",
        report_steps: 150,
        report_dt_days: 2.0,
    },
    Case {
        key: "dep-pvt-correlation",
        note: "dep_pvt scenario base case: constant-rate black-oil blowdown through the bubble point, correlation c_o",
        report_steps: 300,
        report_dt_days: 0.75,
    },
    Case {
        key: "dep-pvt-lab-report",
        note: "dep_pvt scenario lab-report variant: the same blowdown with 2.5x the undersaturated c_o",
        report_steps: 300,
        report_dt_days: 0.75,
    },
];

/// The report schedule actually used: the case's own, or `OPM_SMALL_REPORT_DT` over the same
/// horizon. The override exists for report-step ladders (#21); the deck and the ResSim run both
/// read it, so they cannot disagree.
fn report_schedule(case: &Case) -> (usize, f64) {
    match std::env::var("OPM_SMALL_REPORT_DT")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
    {
        Some(dt) if dt > 0.0 => {
            let horizon = case.report_steps as f64 * case.report_dt_days;
            ((horizon / dt).round() as usize, dt)
        }
        _ => (case.report_steps, case.report_dt_days),
    }
}

fn build(key: &str) -> ReservoirSimulator {
    match key {
        "ow-1d-96" => oil_water(96, 1, [10.0, 10.0, 1.0], (2000.0, 2000.0), 1.0, 0.5, 0),
        "ow-1d-50-adverse" => oil_water(50, 1, [10.0, 10.0, 1.0], (2000.0, 2000.0), 20.0, 0.5, 0),
        "ow-2d-12x12" => oil_water(12, 12, [20.0, 20.0, 5.0], (200.0, 2000.0), 1.0, 0.5, 42),
        "bo-1d-10" => black_oil_depletion(10),
        "bo-1d-40" => black_oil_depletion(40),
        "go-1d-50" => gas_injection_1d(),
        "dep-pvt-correlation" => dep_pvt_column(false),
        "dep-pvt-lab-report" => dep_pvt_column(true),
        other => panic!("unknown small-direct case {other}"),
    }
}

/// Two-phase waterflood: injector in the first cell, producer in the last, both on BHP.
fn oil_water(
    nx: usize,
    ny: usize,
    cell: [f64; 3],
    perm_range: (f64, f64),
    mu_o: f64,
    mu_w: f64,
    seed: u64,
) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(nx, ny, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions(cell[0], cell[1], cell[2]).unwrap();
    if perm_range.0 == perm_range.1 {
        sim.set_permeability_per_layer(vec![perm_range.0], vec![perm_range.0], vec![perm_range.0])
            .unwrap();
    } else {
        sim.set_permeability_random_seeded(perm_range.0, perm_range.1, seed)
            .unwrap();
    }
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_fluid_properties(mu_o, mu_w).unwrap();
    sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_initial_pressure(300.0);
    sim.set_initial_saturation(0.1);
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.add_well_with_id(0, 0, 0, 500.0, 0.1, 0.0, true, "INJ".to_string())
        .unwrap();
    sim.add_well_with_id(
        nx - 1,
        ny - 1,
        0,
        100.0,
        0.1,
        0.0,
        false,
        "PROD".to_string(),
    )
    .unwrap();
    sim
}

/// `tests::physics::depletion_grid_convergence`'s column, verbatim.
fn black_oil_depletion(nx: usize) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(nx, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(1000.0 / nx as f64, 200.0, vec![20.0])
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
    sim.set_initial_pressure(175.0);
    sim.set_initial_saturation(0.10);
    sim.set_initial_gas_saturation(0.0);
    sim.pvt.c_o = 1e-5;
    let row = |p_bar, rs_m3m3, bo_m3m3, mu_o_cp, bg_m3m3, mu_g_cp| PvtRow {
        p_bar,
        rs_m3m3,
        bo_m3m3,
        mu_o_cp,
        bg_m3m3,
        mu_g_cp,
    };
    sim.pvt_table = Some(PvtTable::new(
        vec![
            row(100.0, 5.0, 1.08, 1.5, 0.01, 0.02),
            row(150.0, 15.0, 1.12, 1.2, 0.006, 0.025),
            row(200.0, 15.0, 1.119, 1.3, 0.0045, 0.03),
        ],
        sim.pvt.c_o,
    ));
    sim.set_initial_rs(15.0);
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.injector_enabled = false;
    sim.add_well_with_id(nx - 1, 0, 0, 120.0, 0.1, 0.0, false, "PROD".to_string())
        .unwrap();
    sim
}

/// The `gas_injection` catalog scenario's base case (`src/lib/catalog/scenarios/gas_injection.ts`),
/// configured in the order the worker's `configureReservoirSimulator` applies its payload.
///
/// It has no PVT table: dead oil on `b_o·exp(−c_o·(p − p_ref))` and gas on
/// `Bg = exp(−c_g·(p − p_ref))`, both referenced to the initial pressure (#36, #42).
pub(super) fn gas_injection_1d() -> ReservoirSimulator {
    let nx = 50;
    let mut sim = ReservoirSimulator::new(nx, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions(20.0, 50.0, 10.0).unwrap();
    sim.set_fluid_properties(2.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
    sim.set_rock_properties(1e-6, 0.0, 1.0, 1.0).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_initial_pressure(250.0);
    sim.set_initial_saturation(0.2);
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(false);
    sim.set_rel_perm_props(0.2, 0.15, 2.0, 2.0, 0.4, 1.0)
        .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_three_phase_rel_perm_props(0.2, 0.15, 0.05, 0.05, 0.20, 2.0, 2.0, 1.5, 0.4, 1.0, 0.8)
        .unwrap();
    sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
    sim.set_gas_redissolution_enabled(true);
    sim.set_injected_fluid("gas").unwrap();
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.set_target_well_rates(0.0, 0.0).unwrap();
    sim.set_well_bhp_limits(100.0, 350.0).unwrap();
    sim.set_permeability_per_layer(vec![100.0], vec![100.0], vec![10.0])
        .unwrap();
    sim.add_well_with_id(nx - 1, 0, 0, 100.0, 0.1, 0.0, false, "PROD".to_string())
        .unwrap();
    sim.add_well_with_id(0, 0, 0, 350.0, 0.1, 0.0, true, "INJ".to_string())
        .unwrap();
    sim
}

/// The `dep_pvt` scenario's two PVT tables, exactly as `generateBlackOilTable` makes them.
/// `dep_pvt.test.ts` asserts the scenario still ships these rows, so a change to the correlation
/// shows up there instead of as a deck that no longer matches the scenario.
#[derive(serde::Deserialize)]
struct DepPvtTables {
    correlation: Vec<PvtRow>,
    lab_report: Vec<PvtRow>,
}

const DEP_PVT_TABLES: &str =
    include_str!("../../../../../opm/reference-decks/small-direct/dep-pvt-tables.json");

/// The `dep_pvt` catalog scenario (`src/lib/catalog/scenarios/dep_pvt.ts`), configured in the order
/// the worker's `configureReservoirSimulator` applies its payload. `lab_report` selects the
/// `pvt_lab_report` sensitivity variant: its own table and its own scalar `c_o`.
///
/// The producer is on a surface-oil target of 3 Sm3/day. The scenario's 30 bar `producerBhp` is
/// only the well's BHP target; a rate-controlled producer gets the worker's family BHP floor of 0.
fn dep_pvt_column(lab_report: bool) -> ReservoirSimulator {
    let tables: DepPvtTables =
        serde_json::from_str(DEP_PVT_TABLES).expect("dep-pvt-tables.json parses");
    let (rows, c_o) = if lab_report {
        (tables.lab_report, 2.5e-4)
    } else {
        (tables.correlation, 1.0e-4)
    };
    let nx = 48;
    let mut sim = ReservoirSimulator::new(nx, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions(10.0, 10.0, 10.0).unwrap();
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(c_o, 3e-6).unwrap();
    sim.apply_pvt_table(rows).unwrap();
    sim.set_rock_properties(1e-6, 0.0, 1.1, 1.0).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_initial_pressure(280.0);
    sim.set_initial_saturation(0.1);
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(false);
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_three_phase_rel_perm_props(0.1, 0.1, 0.05, 0.05, 0.15, 2.0, 2.0, 1.5, 1.0, 1.0, 1.0)
        .unwrap();
    sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
    sim.set_gas_redissolution_enabled(true);
    sim.set_injected_fluid("gas").unwrap();
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
    sim.set_target_well_rates(0.0, 3.0).unwrap();
    sim.set_target_well_surface_rates(0.0, 3.0).unwrap();
    sim.set_well_bhp_limits(0.0, 500.0).unwrap();
    sim.set_permeability_per_layer(vec![200.0], vec![200.0], vec![20.0])
        .unwrap();
    sim.add_well_with_id(nx - 1, 0, 0, 30.0, 0.1, 0.0, false, "PROD".to_string())
        .unwrap();
    sim.set_well_schedule(
        "PROD".to_string(),
        "rate".to_string(),
        3.0,
        3.0,
        f64::NAN,
        true,
    )
    .unwrap();
    sim
}

// ---- deck ---------------------------------------------------------------------------------

fn values(out: &mut String, keyword: &str, data: impl IntoIterator<Item = f64>) {
    let _ = writeln!(out, "{keyword}");
    for (idx, value) in data.into_iter().enumerate() {
        let _ = write!(out, " {value:.10e}");
        if idx % 6 == 5 {
            out.push('\n');
        }
    }
    out.push_str(" /\n");
}

/// Saturation grid for a table: `points` evenly spaced nodes plus the curve's own endpoints, so
/// the kinks of a Corey curve sit on table nodes rather than between them.
fn saturation_nodes(lo: f64, hi: f64, points: usize, kinks: &[f64]) -> Vec<f64> {
    let mut nodes: Vec<f64> = (0..points)
        .map(|i| lo + (hi - lo) * i as f64 / (points - 1) as f64)
        .chain(kinks.iter().copied().filter(|k| *k > lo && *k < hi))
        .collect();
    nodes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    nodes.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    nodes
}

fn deck(case: &Case, sim: &ReservoirSimulator) -> String {
    let (nx, ny, nz) = (sim.nx, sim.ny, sim.nz);
    let n = nx * ny * nz;
    let three_phase = sim.three_phase_mode;
    // Three-phase with a PVT table is live black oil; without one it is dead oil plus dry gas.
    let black_oil = three_phase && sim.pvt_table.is_some();
    let gas_injector = three_phase
        && matches!(sim.injected_fluid, crate::InjectedFluid::Gas)
        && sim.wells.iter().any(|well| well.injector);
    let mut d = String::new();
    let _ = writeln!(
        d,
        "-- Generated by src/lib/ressim/src/tests/opm_small_direct.rs; do not edit by hand.\n\
         -- Case {}: {}.\n\
         -- Tables are ResSim's own relperm/PVT functions sampled onto dense nodes.\n\
         -- Run with: flow CASE.DATA --enable-gravity=false",
        case.key, case.note
    );
    d.push_str("RUNSPEC\n");
    let _ = writeln!(d, "TITLE\n  ressim small-direct {} /", case.key);
    let _ = writeln!(d, "DIMENS\n  {nx} {ny} {nz} /");
    d.push_str(if black_oil {
        "OIL\nWATER\nGAS\nDISGAS\n"
    } else if three_phase {
        "OIL\nWATER\nGAS\n"
    } else {
        "OIL\nWATER\n"
    });
    d.push_str("METRIC\nTABDIMS\n  1 1 250 250 1 250 /\nWELLDIMS\n  2 1 1 2 /\n");
    d.push_str("START\n  1 JAN 2026 /\nUNIFOUT\n");
    d.push_str("GRID\nINIT\n");
    values(&mut d, "DX", (0..n).map(|_| sim.dx));
    values(&mut d, "DY", (0..n).map(|_| sim.dy));
    values(&mut d, "DZ", (0..n).map(|id| sim.dz[id / (nx * ny)]));
    values(&mut d, "TOPS", (0..nx * ny).map(|_| 1000.0));
    values(&mut d, "PORO", (0..n).map(|id| sim.porosity[id]));
    values(&mut d, "PERMX", (0..n).map(|id| sim.perm_x[id]));
    values(&mut d, "PERMY", (0..n).map(|id| sim.perm_y[id]));
    values(&mut d, "PERMZ", (0..n).map(|id| sim.perm_z[id]));

    d.push_str("PROPS\n");
    let _ = writeln!(
        d,
        "PVTW\n  {} {} {:e} {} 0 /",
        sim.water_pvt_reference_pressure_bar, sim.b_w, sim.pvt.c_w, sim.pvt.mu_w
    );
    let _ = writeln!(
        d,
        "ROCK\n  {} {:e} /",
        sim.rock_reference_pressure_bar, sim.rock_compressibility
    );
    if three_phase && !black_oil {
        let _ = writeln!(
            d,
            "DENSITY\n  {} {} {} /",
            sim.pvt.rho_o, sim.pvt.rho_w, sim.rho_g
        );
    } else {
        d.push_str("DENSITY\n  800 1000 0.9 /\n");
    }

    if three_phase {
        let scal = sim.scal_3p.as_ref().expect("three-phase case has scal_3p");
        d.push_str("STONE2\nSWOF\n");
        for sw in saturation_nodes(scal.s_wc, 1.0, 91, &[1.0 - scal.s_or]) {
            let _ = writeln!(
                d,
                "  {sw:.8} {:.10e} {:.10e} 0",
                scal.k_rw(sw),
                scal.k_ro_water(sw)
            );
        }
        d.push_str("/\nSGOF\n");
        let sg_max = 1.0 - scal.s_wc;
        for sg in saturation_nodes(0.0, sg_max, 91, &[scal.s_gc, 1.0 - scal.s_wc - scal.s_org]) {
            let _ = writeln!(
                d,
                "  {sg:.8} {:.10e} {:.10e} 0",
                scal.k_rg(sg),
                scal.k_ro_gas(sg)
            );
        }
        d.push_str("/\n");
    }
    if three_phase && !black_oil {
        d.push_str("PVDO\n");
        for step in 0..=24 {
            let p = 50.0 + 25.0 * step as f64;
            let _ = writeln!(
                d,
                "  {p:.4} {:.10e} {:.10e}",
                sim.get_b_o_cell(0, p),
                sim.get_mu_o(p)
            );
        }
        // ResSim's table-less gas, `Bg = exp(−c_g·(p − p_ref))` (#42), sampled like the oil.
        d.push_str("/\nPVDG\n");
        for step in 0..=24 {
            let p = 50.0 + 25.0 * step as f64;
            let _ = writeln!(
                d,
                "  {p:.4} {:.10e} {:.10e}",
                sim.get_b_g(p),
                sim.get_mu_g(p)
            );
        }
        d.push_str("/\n");
    }
    if black_oil {
        let table = sim
            .pvt_table
            .as_ref()
            .expect("black-oil case has a PVT table");
        // Pressure nodes about every 2.5 bar across the table, with the table's own rows (the
        // bubble point among them) on nodes. Saturated oil sits on every node where Rs still
        // rises, each with an undersaturated branch evaluated by ResSim's own `interpolate_oil`,
        // so Flow's interpolation rule only acts between nodes a few bar apart.
        let (p_lo, p_hi) = table
            .rows
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |acc, row| {
                (acc.0.min(row.p_bar), acc.1.max(row.p_bar))
            });
        let row_pressures: Vec<f64> = table.rows.iter().map(|row| row.p_bar).collect();
        let pressure_nodes = saturation_nodes(
            p_lo,
            p_hi,
            ((p_hi - p_lo) / 2.5).round() as usize + 1,
            &row_pressures,
        );
        d.push_str("PVTO\n");
        let mut last_rs = f64::NEG_INFINITY;
        for &p in &pressure_nodes {
            let sat = table.interpolate(p);
            if sat.rs_m3m3 <= last_rs + 1e-9 {
                continue;
            }
            last_rs = sat.rs_m3m3;
            let (bo, mu) = table.interpolate_oil(p, sat.rs_m3m3);
            let _ = writeln!(d, "  {:.8} {p:.4} {bo:.10e} {mu:.10e}", sat.rs_m3m3);
            // Each branch reaches the top of the table, so Flow never extrapolates one.
            let mut offsets = vec![10.0, 25.0, 50.0, 100.0];
            if p + 100.0 < p_hi {
                offsets.push(p_hi - p);
            }
            for dp in offsets {
                let (bo_u, mu_u) = table.interpolate_oil(p + dp, sat.rs_m3m3);
                let _ = writeln!(d, "           {:.4} {bo_u:.10e} {mu_u:.10e}", p + dp);
            }
            d.push_str("  /\n");
        }
        d.push_str("/\nPVDG\n");
        for &p in &pressure_nodes {
            let row = table.interpolate(p);
            let _ = writeln!(d, "  {p:.4} {:.10e} {:.10e}", row.bg_m3m3, row.mu_g_cp);
        }
        d.push_str("/\n");
    }
    if !three_phase {
        d.push_str("SWOF\n");
        for sw in saturation_nodes(sim.scal.s_wc, 1.0, 161, &[1.0 - sim.scal.s_or]) {
            let _ = writeln!(
                d,
                "  {sw:.8} {:.10e} {:.10e} 0",
                sim.scal.k_rw(sw),
                sim.scal.k_ro(sw)
            );
        }
        d.push_str("/\nPVDO\n");
        for step in 0..=24 {
            let p = 50.0 + 25.0 * step as f64;
            let _ = writeln!(
                d,
                "  {p:.4} {:.10e} {:.10e}",
                sim.get_b_o_cell(0, p),
                sim.pvt.mu_o
            );
        }
        d.push_str("/\n");
    }

    d.push_str("SOLUTION\n");
    values(&mut d, "PRESSURE", sim.pressure.iter().copied());
    values(&mut d, "SWAT", sim.sat_water.iter().copied());
    if three_phase {
        values(&mut d, "SGAS", sim.sat_gas.iter().copied());
    }
    if black_oil {
        values(&mut d, "RS", sim.rs.iter().copied());
    }

    // FPR and FVIT (reservoir-volume injection) are what the frontend artifacts of
    // `tools/opm_flow` need; RUNSUM/SEPARATE make Flow write the text summary those read.
    d.push_str("SUMMARY\nFOPR\nFWPR\nFWIR\nFOPT\nFWPT\nFWIT\nFPR\n");
    if three_phase {
        d.push_str("FGPR\nFGPT\nFGOR\n");
    }
    if gas_injector {
        d.push_str("FGIR\nFGIT\n");
    }
    if sim.wells.iter().any(|well| well.injector) {
        d.push_str("FVIT\n");
    }
    d.push_str("WBHP\n/\nRUNSUM\nSEPARATE\n");

    d.push_str("SCHEDULE\nRPTRST\n  BASIC=2 /\n");
    if black_oil && !sim.gas_redissolution_enabled {
        d.push_str("DRSDT\n  0 /\n");
    }
    d.push_str("WELSPECS\n");
    for well in &sim.wells {
        let name = well.physical_well_id.as_deref().unwrap_or("W");
        let phase = match (well.injector, gas_injector) {
            (true, true) => "GAS",
            (true, false) => "WATER",
            (false, _) => "OIL",
        };
        let _ = writeln!(
            d,
            "  '{name}' 'G' {} {} 1* '{phase}' /",
            well.i + 1,
            well.j + 1
        );
    }
    d.push_str("/\nCOMPDAT\n");
    for well in &sim.wells {
        let name = well.physical_well_id.as_deref().unwrap_or("W");
        // Items 7-8 defaulted so Flow computes the Peaceman factor; item 9 is a DIAMETER.
        let _ = writeln!(
            d,
            "  '{name}' {} {} {} {} 'OPEN' 2* {} /",
            well.i + 1,
            well.j + 1,
            well.k + 1,
            well.k + 1,
            2.0 * well.well_radius
        );
    }
    d.push_str("/\n");
    for well in &sim.wells {
        let name = well.physical_well_id.as_deref().unwrap_or("W");
        if well.injector {
            let _ = writeln!(
                d,
                "WCONINJE\n  '{name}' '{}' 'OPEN' 'BHP' 1* 1* {} /\n/",
                if gas_injector { "GAS" } else { "WATER" },
                well.bhp
            );
        } else {
            let control = sim.well_control_config(well);
            match (control.rate_controlled, control.target_surface_rate_m3_day) {
                // A producer's surface target is stock-tank oil. Flow needs a positive BHP
                // limit; ResSim's floor of 0 means "none", and 1 bar never binds either.
                (true, Some(target)) => {
                    let _ = writeln!(
                        d,
                        "WCONPROD\n  '{name}' 'OPEN' 'ORAT' {target} 4* {} /\n/",
                        control.bhp_limit.max(1.0)
                    );
                }
                (true, None) => panic!("{name}: reservoir-volume producer targets are not written"),
                (false, _) => {
                    let _ = writeln!(d, "WCONPROD\n  '{name}' 'OPEN' 'BHP' 5* {} /\n/", well.bhp);
                }
            }
        }
    }
    let _ = writeln!(
        d,
        "TSTEP\n  {}*{} /\nEND",
        report_schedule(case).0,
        report_schedule(case).1
    );
    d
}

#[test]
#[ignore = "writes the committed small-direct OPM decks; set OPM_SMALL_DECK_DIR"]
fn opm_small_direct_write_decks() {
    let dir = std::env::var("OPM_SMALL_DECK_DIR").expect("set OPM_SMALL_DECK_DIR");
    for case in &CASES {
        let sim = build(case.key);
        let path = std::path::Path::new(&dir).join(case.key);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("CASE.DATA"), deck(case, &sim)).unwrap();
    }
}

// ---- ResSim run ---------------------------------------------------------------------------

fn json_array(values: &[f64]) -> String {
    let parts: Vec<String> = values.iter().map(|v| format!("{v:.12e}")).collect();
    format!("[{}]", parts.join(","))
}

/// Which ResSim solver a run uses: FIM on one small-system LU, or IMPES. The id names the output
/// file, `<case>.<id>.json`, and is the column `compare_small_direct.py` reports it under.
fn run_id(solver: &str, backend: &str) -> String {
    match solver {
        "fim" => backend.to_string(),
        "impes" => "impes".to_string(),
        other => panic!("OPM_SMALL_SOLVER must be fim or impes, not {other}"),
    }
}

#[test]
#[ignore = "runs the small-direct cases through ResSim; set OPM_SMALL_OUT, OPM_SMALL_SOLVER and OPM_SMALL_BACKEND"]
fn opm_small_direct_run_ressim() {
    let out = std::env::var("OPM_SMALL_OUT").expect("set OPM_SMALL_OUT");
    let solver = std::env::var("OPM_SMALL_SOLVER").unwrap_or_else(|_| "fim".to_string());
    let backend = std::env::var("OPM_SMALL_BACKEND").unwrap_or_else(|_| "sparse".to_string());
    let id = run_id(&solver, &backend);
    let only = std::env::var("OPM_SMALL_CASE").ok();
    std::fs::create_dir_all(&out).unwrap();
    for case in CASES
        .iter()
        .filter(|c| only.as_deref().is_none_or(|k| k == c.key))
    {
        let mut sim = build(case.key);
        let fim = solver == "fim";
        sim.set_fim_enabled(fim);
        if fim {
            sim.set_fim_direct_backend(backend.clone()).unwrap();
        }
        let started = Instant::now();
        let mut reports = Vec::new();
        // A warning is recorded, not asserted: the comparison is where a run is judged, and a
        // diagnostic that aborts on the first warning hides everything after it.
        let mut warnings: Vec<String> = Vec::new();
        let (report_steps, report_dt_days) = report_schedule(case);
        for _ in 0..report_steps {
            let history_before = sim.rate_history.len();
            sim.step(report_dt_days);
            if !sim.last_solver_warning.is_empty() {
                warnings.push(format!(
                    "t={:.4}: {}",
                    sim.time_days, sim.last_solver_warning
                ));
            }
            // Every accepted substep appends one history row, on both solvers.
            let substeps = sim.rate_history.len() - history_before;
            // IMPES has no Newton loop and no retry ladder: its counters are null, not zero.
            let counters = if fim {
                let stats = sim
                    .last_fim_step_stats
                    .clone()
                    .expect("FIM step records stats");
                let newton: usize = stats
                    .accepted_rungs
                    .iter()
                    .flatten()
                    .map(|r| r.newton_iterations)
                    .sum();
                let retry_newton: usize = stats
                    .retry_rungs
                    .iter()
                    .flatten()
                    .map(|r| r.newton_iterations)
                    .sum();
                format!(
                    "\"retries\":{},\"newton\":{newton},\"retry_newton\":{retry_newton}",
                    stats.linear_bad_retries + stats.nonlinear_bad_retries + stats.mixed_retries,
                )
            } else {
                "\"retries\":null,\"newton\":null,\"retry_newton\":null".to_string()
            };
            reports.push(format!(
                "{{\"t\":{:.10},\"substeps\":{substeps},{counters},\
                 \"p\":{},\"sw\":{},\"sg\":{},\"rs\":{}}}",
                sim.time_days,
                json_array(&sim.pressure),
                json_array(&sim.sat_water),
                json_array(&sim.sat_gas),
                json_array(&sim.rs),
            ));
        }
        let wall_ms = started.elapsed().as_secs_f64() * 1e3;
        let history: Vec<String> = sim
            .rate_history
            .iter()
            .map(|r| {
                format!(
                    "[{:.10},{:.10e},{:.10e},{:.10e},{:.10e}]",
                    r.time,
                    r.total_production_oil,
                    r.total_production_liquid - r.total_production_oil,
                    r.total_production_gas,
                    r.total_injection
                )
            })
            .collect();
        let json = format!(
            "{{\"case\":\"{}\",\"solver\":\"{solver}\",\"backend\":\"{}\",\"run\":\"{id}\",\
             \"wall_ms\":{wall_ms:.3},\"injected\":\"{}\",\"warnings\":{},\
             \"history_columns\":[\"t\",\"qo\",\"qw\",\"qg\",\"qwi\"],\
             \"history\":[{}],\"reports\":[{}]}}\n",
            case.key,
            if fim { backend.as_str() } else { "" },
            if sim.three_phase_mode && matches!(sim.injected_fluid, crate::InjectedFluid::Gas) {
                "gas"
            } else {
                "water"
            },
            serde_json::to_string(&warnings).expect("warnings serialize"),
            history.join(","),
            reports.join(",")
        );
        std::fs::write(
            std::path::Path::new(&out).join(format!("{}.{id}.json", case.key)),
            json,
        )
        .unwrap();
        let substeps: usize = sim.rate_history.len();
        println!(
            "{:<20} {id:<6} substeps={substeps:<7} warnings={:<3} wall={wall_ms:.1} ms",
            case.key,
            warnings.len()
        );
    }
}
