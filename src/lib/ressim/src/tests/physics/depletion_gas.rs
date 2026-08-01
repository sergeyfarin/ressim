use super::fixtures::{
    make_closed_gas_depletion_single_cell_sim, total_gas_inventory_sc_all_cells,
};
use crate::pvt::{PvtRow, PvtTable};

#[derive(Clone, Copy)]
struct GasDepletionCase {
    name: &'static str,
    initial_sw: f64,
    initial_sg: f64,
    perm_md: f64,
    mu_g_cp: f64,
    c_g: f64,
    bg_scale: f64,
    n_g: f64,
}

fn make_gas_depletion_case(case: GasDepletionCase) -> crate::ReservoirSimulator {
    let mut sim = make_closed_gas_depletion_single_cell_sim();
    sim.set_three_phase_rel_perm_props(
        0.10, 0.10, 0.03, 0.03, 0.10, 2.0, 2.0, case.n_g, 0.8, 0.9, 0.7,
    )
    .unwrap();
    sim.set_gas_fluid_properties(case.mu_g_cp, case.c_g, 10.0)
        .unwrap();
    sim.set_initial_saturation(case.initial_sw);
    sim.set_initial_gas_saturation(case.initial_sg);
    sim.set_permeability_per_layer(vec![case.perm_md], vec![case.perm_md], vec![case.perm_md])
        .unwrap();
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 100.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.05,
                mu_o_cp: 1.1,
                bg_m3m3: 0.012 * case.bg_scale,
                mu_g_cp: case.mu_g_cp,
            },
            PvtRow {
                p_bar: 200.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.02,
                mu_o_cp: 1.1,
                bg_m3m3: 0.006 * case.bg_scale,
                mu_g_cp: case.mu_g_cp * 1.05,
            },
            PvtRow {
                p_bar: 300.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.00,
                mu_o_cp: 1.1,
                bg_m3m3: 0.004 * case.bg_scale,
                mu_g_cp: case.mu_g_cp * 1.10,
            },
        ],
        sim.pvt.c_o,
    ));
    sim
}

fn cumulative_gas_production_sc(sim: &crate::ReservoirSimulator) -> f64 {
    let mut cumulative_gas = 0.0;
    let mut previous_time_days = 0.0;

    for point in &sim.rate_history {
        let dt_days = point.time - previous_time_days;
        previous_time_days = point.time;
        cumulative_gas += point.total_production_gas.max(0.0) * dt_days;
    }

    cumulative_gas
}

#[test]
fn physics_depletion_gas_single_cell_timestep_stable() {
    let mut coarse = make_closed_gas_depletion_single_cell_sim();
    coarse.step(0.01);
    assert!(
        coarse.last_solver_warning.is_empty(),
        "single-cell coarse gas depletion emitted solver warning: {}",
        coarse.last_solver_warning
    );
    let coarse_point = coarse
        .rate_history
        .last()
        .expect("single-cell coarse gas depletion should record history");

    let mut fine = make_closed_gas_depletion_single_cell_sim();
    fine.step(0.005);
    fine.step(0.005);
    assert!(
        fine.last_solver_warning.is_empty(),
        "single-cell fine gas depletion emitted solver warning: {}",
        fine.last_solver_warning
    );
    let fine_point = fine
        .rate_history
        .last()
        .expect("single-cell fine gas depletion should record history");

    let gas_rate_rel_diff = ((coarse_point.total_production_gas - fine_point.total_production_gas)
        / fine_point.total_production_gas.max(1e-12))
    .abs();
    let avg_pressure_rel_diff = ((coarse_point.avg_reservoir_pressure
        - fine_point.avg_reservoir_pressure)
        / fine_point.avg_reservoir_pressure.max(1e-12))
    .abs();

    assert!(
        gas_rate_rel_diff <= 0.05,
        "single-cell gas depletion timestep gas-rate drift too large: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_point.total_production_gas,
        fine_point.total_production_gas,
        gas_rate_rel_diff,
    );
    assert!(
        avg_pressure_rel_diff <= 0.01,
        "single-cell gas depletion timestep pressure drift too large: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_point.avg_reservoir_pressure,
        fine_point.avg_reservoir_pressure,
        avg_pressure_rel_diff,
    );
}

#[test]
fn physics_depletion_gas_single_cell_runs_with_positive_gas_rate() {
    let mut sim = make_closed_gas_depletion_single_cell_sim();

    sim.step(0.01);
    assert!(
        sim.last_solver_warning.is_empty(),
        "single-cell gas depletion emitted solver warning: {}",
        sim.last_solver_warning
    );

    let latest = sim
        .rate_history
        .last()
        .expect("single-cell gas depletion should record history");

    assert!(latest.total_production_gas > 0.0);
    assert!(latest.avg_reservoir_pressure.is_finite());
    assert!(sim.sat_gas[0].is_finite());
    assert!(sim.sat_gas[0] >= 0.0);
}

#[test]
fn physics_depletion_gas_single_cell_storage_and_material_balance_close() {
    let mut sim = make_closed_gas_depletion_single_cell_sim();
    let initial_inventory_sc = total_gas_inventory_sc_all_cells(&sim);

    for _ in 0..8 {
        sim.step(0.005);
        assert!(
            sim.last_solver_warning.is_empty(),
            "single-cell gas depletion storage case emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    let latest = sim
        .rate_history
        .last()
        .expect("single-cell gas depletion storage case should record history");
    let final_inventory_sc = total_gas_inventory_sc_all_cells(&sim);
    let cumulative_gas_sc = cumulative_gas_production_sc(&sim);
    let accounted_gas_sc = final_inventory_sc + cumulative_gas_sc;
    let gas_balance_rel_diff =
        ((accounted_gas_sc - initial_inventory_sc) / initial_inventory_sc.max(1e-12)).abs();

    assert!(
        latest.total_production_gas > 0.0,
        "single-cell gas depletion should keep producing gas, got {}",
        latest.total_production_gas
    );
    assert!(
        final_inventory_sc <= initial_inventory_sc + 1e-4,
        "single-cell gas depletion should not create in-place gas inventory: initial={:.6}, final={:.6}",
        initial_inventory_sc,
        final_inventory_sc
    );
    assert!(
        latest.material_balance_error_gas_m3.is_finite(),
        "single-cell gas depletion gas MB diagnostic should stay finite, got {}",
        latest.material_balance_error_gas_m3
    );
    assert!(
        gas_balance_rel_diff <= 0.15,
        "single-cell gas depletion material balance envelope drift too large: initial={:.6}, final+prod={:.6}, rel_diff={:.4}",
        initial_inventory_sc,
        accounted_gas_sc,
        gas_balance_rel_diff
    );
}

#[test]
fn physics_depletion_gas_gentle_case_keeps_per_step_material_balance_tight() {
    let mut sim = make_closed_gas_depletion_single_cell_sim();
    sim.set_permeability_per_layer(vec![20.0], vec![20.0], vec![20.0])
        .unwrap();
    sim.wells[0].bhp = 210.0;

    let initial_inventory_sc = total_gas_inventory_sc_all_cells(&sim);

    for _ in 0..5 {
        sim.step(0.005);
        assert!(
            sim.last_solver_warning.is_empty(),
            "gentle gas depletion MB case emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );

        let gas_inventory_sc = total_gas_inventory_sc_all_cells(&sim);
        let cumulative_gas_sc = cumulative_gas_production_sc(&sim);
        let accounted_gas_sc = gas_inventory_sc + cumulative_gas_sc;
        let gas_balance_rel_diff =
            ((accounted_gas_sc - initial_inventory_sc) / initial_inventory_sc.max(1e-12)).abs();

        assert!(
            gas_balance_rel_diff <= 1e-3,
            "gentle gas depletion per-step MB drift too large at t={:.4}: initial={:.6}, inventory+prod={:.6}, rel_diff={:.6}",
            sim.time_days,
            initial_inventory_sc,
            accounted_gas_sc,
            gas_balance_rel_diff
        );
    }
}

#[test]
fn physics_depletion_gas_single_cell_storage_tracks_bg_response() {
    let mut high_pressure = make_closed_gas_depletion_single_cell_sim();
    let mut low_pressure = make_closed_gas_depletion_single_cell_sim();

    high_pressure.pressure[0] = 300.0;
    low_pressure.pressure[0] = 100.0;

    let high_pressure_inventory_sc = total_gas_inventory_sc_all_cells(&high_pressure);
    let low_pressure_inventory_sc = total_gas_inventory_sc_all_cells(&low_pressure);

    assert!(
        high_pressure_inventory_sc > low_pressure_inventory_sc,
        "single-cell gas storage should scale with Bg(p): high-pressure inventory={:.6}, low-pressure inventory={:.6}",
        high_pressure_inventory_sc,
        low_pressure_inventory_sc
    );
}

#[test]
fn physics_depletion_gas_single_cell_closed_system_monotone() {
    let mut sim = make_closed_gas_depletion_single_cell_sim();
    let mut prev_gas_inventory_sc = total_gas_inventory_sc_all_cells(&sim);
    let mut prev_pressure = sim.pressure[0];

    for _ in 0..8 {
        sim.step(0.005);
        assert!(
            sim.last_solver_warning.is_empty(),
            "single-cell gas depletion emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );

        let latest = sim
            .rate_history
            .last()
            .expect("gas depletion should record history");
        let gas_inventory_sc = total_gas_inventory_sc_all_cells(&sim);

        assert!(
            latest.total_production_gas > 0.0,
            "gas rate must remain positive during closed depletion at t={}",
            sim.time_days
        );
        assert!(
            latest.avg_reservoir_pressure <= prev_pressure + 1e-9,
            "pressure must not increase during closed gas depletion: prev={:.4}, now={:.4}",
            prev_pressure,
            latest.avg_reservoir_pressure
        );
        assert!(
            gas_inventory_sc <= prev_gas_inventory_sc + 1e-4,
            "gas SC inventory must not increase during closed depletion: prev={:.4}, now={:.4}",
            prev_gas_inventory_sc,
            gas_inventory_sc
        );

        prev_gas_inventory_sc = gas_inventory_sc;
        prev_pressure = latest.avg_reservoir_pressure;
    }
}

#[test]
fn physics_depletion_gas_public_invariants_hold_on_both_solvers() {
    fn run_case(fim_enabled: bool) -> (f64, f64, f64, f64, f64, f64, f64, usize) {
        let mut sim = make_closed_gas_depletion_single_cell_sim();
        sim.set_fim_enabled(fim_enabled);

        let initial_inventory_sc = total_gas_inventory_sc_all_cells(&sim);
        let mut prev_gas_inventory_sc = initial_inventory_sc;
        let mut prev_pressure = sim.pressure[0];

        for _ in 0..8 {
            sim.step(0.005);
            assert!(
                sim.last_solver_warning.is_empty(),
                "two-solver gas depletion public-contract case emitted solver warning for fim_enabled={}: {}",
                fim_enabled,
                sim.last_solver_warning
            );

            let latest = sim
                .rate_history
                .last()
                .expect("two-solver gas depletion public-contract case should record history");
            let gas_inventory_sc = total_gas_inventory_sc_all_cells(&sim);

            assert!(latest.total_injection.abs() <= 1e-12);
            assert!(latest.total_production_gas > 0.0);
            assert!(latest.avg_reservoir_pressure.is_finite());
            assert!(latest.producing_gor.is_finite());
            assert!(latest.material_balance_error_gas_m3.is_finite());
            assert!(
                latest.avg_reservoir_pressure <= prev_pressure + 1e-9,
                "gas depletion pressure should not increase for fim_enabled={}: prev={:.6}, now={:.6}",
                fim_enabled,
                prev_pressure,
                latest.avg_reservoir_pressure
            );
            assert!(
                gas_inventory_sc <= prev_gas_inventory_sc + 1e-4,
                "gas depletion inventory should not increase for fim_enabled={}: prev={:.6}, now={:.6}",
                fim_enabled,
                prev_gas_inventory_sc,
                gas_inventory_sc
            );
            assert!(sim.sat_gas[0].is_finite());
            assert!(sim.sat_gas[0] >= -1e-9 && sim.sat_gas[0] <= 1.0 + 1e-9);

            prev_gas_inventory_sc = gas_inventory_sc;
            prev_pressure = latest.avg_reservoir_pressure;
        }

        let latest = sim
            .rate_history
            .last()
            .expect("two-solver gas depletion public-contract case should record history");
        let final_inventory_sc = total_gas_inventory_sc_all_cells(&sim);
        let cumulative_gas_sc = cumulative_gas_production_sc(&sim);
        let accounting_rel_diff = ((final_inventory_sc + cumulative_gas_sc - initial_inventory_sc)
            / initial_inventory_sc.max(1e-12))
        .abs();

        (
            final_inventory_sc,
            cumulative_gas_sc,
            accounting_rel_diff,
            latest.producer_bhp_limited_fraction,
            latest.injector_bhp_limited_fraction,
            latest.time,
            sim.sat_gas[0],
            sim.rate_history.len(),
        )
    }

    for (fim_enabled, metrics) in [(false, run_case(false)), (true, run_case(true))] {
        assert!(metrics.0 >= 0.0);
        assert!(
            metrics.1 > 0.0,
            "expected cumulative gas production for fim_enabled={}",
            fim_enabled
        );
        assert!(
            metrics.2 <= 1.5e-1,
            "gas depletion accounting envelope too large for fim_enabled={}: {}",
            fim_enabled,
            metrics.2
        );
        assert!((0.0..=1.0).contains(&metrics.3));
        assert!((0.0..=1.0).contains(&metrics.4));
        assert!((metrics.5 - 0.04).abs() <= 1e-9);
        assert!(metrics.6.is_finite());
        assert!(
            metrics.7 > 0,
            "expected rate history for fim_enabled={}",
            fim_enabled
        );
    }
}

#[test]
fn physics_depletion_gas_case_matrix_stays_physical_across_sat_perm_pvt_and_scal_ranges() {
    let cases = [
        GasDepletionCase {
            name: "gas-rich base",
            initial_sw: 0.08,
            initial_sg: 0.89,
            perm_md: 500.0,
            mu_g_cp: 0.02,
            c_g: 1e-4,
            bg_scale: 1.0,
            n_g: 1.5,
        },
        GasDepletionCase {
            name: "lower perm steeper gas relperm",
            initial_sw: 0.15,
            initial_sg: 0.80,
            perm_md: 75.0,
            mu_g_cp: 0.03,
            c_g: 2e-4,
            bg_scale: 1.2,
            n_g: 2.0,
        },
        GasDepletionCase {
            name: "high perm lighter gas",
            initial_sw: 0.05,
            initial_sg: 0.92,
            perm_md: 2_000.0,
            mu_g_cp: 0.015,
            c_g: 5e-5,
            bg_scale: 0.85,
            n_g: 1.2,
        },
    ];

    for case in cases {
        let mut sim = make_gas_depletion_case(case);
        let initial_pressure = sim.pressure[0];
        let initial_inventory_sc = total_gas_inventory_sc_all_cells(&sim);

        sim.step(0.005);
        sim.step(0.005);

        assert!(
            sim.last_solver_warning.is_empty(),
            "{} gas depletion emitted solver warning at t={}: {}",
            case.name,
            sim.time_days,
            sim.last_solver_warning
        );

        let latest = sim
            .rate_history
            .last()
            .expect("gas depletion case matrix should record history");
        let final_inventory_sc = total_gas_inventory_sc_all_cells(&sim);

        assert!(
            latest.total_production_gas > 0.0,
            "{} should keep positive gas production, got {}",
            case.name,
            latest.total_production_gas
        );
        assert!(
            latest.avg_reservoir_pressure <= initial_pressure + 1e-9,
            "{} should not increase pressure: initial={:.6}, final={:.6}",
            case.name,
            initial_pressure,
            latest.avg_reservoir_pressure
        );
        assert!(
            final_inventory_sc <= initial_inventory_sc + 1e-4,
            "{} should not create gas inventory: initial={:.6}, final={:.6}",
            case.name,
            initial_inventory_sc,
            final_inventory_sc
        );
        assert!(
            sim.sat_gas[0].is_finite() && sim.sat_gas[0] >= -1e-9 && sim.sat_gas[0] <= 1.0 + 1e-9,
            "{} gas saturation escaped bounds: {}",
            case.name,
            sim.sat_gas[0]
        );
    }
}

#[test]
#[ignore = "explicit refinement probe: single-cell gas depletion should stay stable under a longer coarse-vs-fine horizon"]
fn physics_depletion_gas_single_cell_timestep_refinement_keeps_inventory_stable() {
    let mut coarse = make_closed_gas_depletion_single_cell_sim();
    let mut fine = make_closed_gas_depletion_single_cell_sim();

    for _ in 0..8 {
        coarse.step(0.005);
        assert!(
            coarse.last_solver_warning.is_empty(),
            "coarse single-cell gas depletion emitted solver warning at t={}: {}",
            coarse.time_days,
            coarse.last_solver_warning
        );
    }
    for _ in 0..16 {
        fine.step(0.0025);
        assert!(
            fine.last_solver_warning.is_empty(),
            "fine single-cell gas depletion emitted solver warning at t={}: {}",
            fine.time_days,
            fine.last_solver_warning
        );
    }

    let coarse_last = coarse
        .rate_history
        .last()
        .expect("coarse gas depletion should record history");
    let fine_last = fine
        .rate_history
        .last()
        .expect("fine gas depletion should record history");
    let coarse_inventory = total_gas_inventory_sc_all_cells(&coarse);
    let fine_inventory = total_gas_inventory_sc_all_cells(&fine);
    let coarse_cum_gas = cumulative_gas_production_sc(&coarse);
    let fine_cum_gas = cumulative_gas_production_sc(&fine);

    let pressure_rel_diff = ((coarse_last.avg_reservoir_pressure
        - fine_last.avg_reservoir_pressure)
        / fine_last.avg_reservoir_pressure.max(1e-12))
    .abs();
    let inventory_rel_diff =
        ((coarse_inventory - fine_inventory) / fine_inventory.max(1e-12)).abs();
    let cumulative_gas_rel_diff = ((coarse_cum_gas - fine_cum_gas) / fine_cum_gas.max(1e-12)).abs();

    assert!(
        pressure_rel_diff <= 0.02,
        "single-cell gas depletion avg-pressure drift too large under timestep refinement: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_last.avg_reservoir_pressure,
        fine_last.avg_reservoir_pressure,
        pressure_rel_diff
    );
    assert!(
        inventory_rel_diff <= 0.03,
        "single-cell gas depletion inventory drift too large under timestep refinement: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_inventory,
        fine_inventory,
        inventory_rel_diff
    );
    assert!(
        cumulative_gas_rel_diff <= 0.05,
        "single-cell gas depletion cumulative-gas drift too large under timestep refinement: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_cum_gas,
        fine_cum_gas,
        cumulative_gas_rel_diff
    );
}

/// A deep dry-gas tank with an ideal-gas PVT table, for the compaction tests.
///
/// `B_g = C/p` (z = 1) makes `1/B_g` exactly proportional to pressure, so the
/// order-of-magnitude contrast in surface gas density between initial and
/// abandonment conditions — which is what makes the pore-volume reference
/// matter — is present and analytically clean.
fn make_deep_dry_gas_tank(rock_compressibility: f64) -> crate::ReservoirSimulator {
    let mut sim = crate::ReservoirSimulator::new(1, 1, 1, 0.15);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(300.0, 300.0, vec![20.0])
        .unwrap();
    sim.set_three_phase_rel_perm_props(0.2, 0.0, 0.02, 0.02, 0.0, 2.0, 2.0, 1.5, 0.4, 1.0, 0.9)
        .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_gas_redissolution_enabled(false);
    sim.set_gas_fluid_properties(0.02, 1e-4, 0.8).unwrap();
    sim.set_fluid_properties(1.0, 0.4).unwrap();
    sim.set_fluid_compressibilities(1e-5, 4e-5).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.1, 75.0, 0.75);
    sim.set_permeability_per_layer(vec![500.0], vec![500.0], vec![500.0])
        .unwrap();

    // Ideal gas: B_g = 4.0 / p, so 1/B_g runs from 100 at 400 bar to 5 at 20.
    let rows: Vec<PvtRow> = (1..=41)
        .map(|i| {
            let p_bar = f64::from(i) * 10.0;
            PvtRow {
                p_bar,
                rs_m3m3: 0.0,
                bo_m3m3: 1.05,
                mu_o_cp: 1.1,
                bg_m3m3: 4.0 / p_bar,
                mu_g_cp: 0.02,
            }
        })
        .collect();
    sim.pvt_table = Some(PvtTable::new(rows, 1e-5));

    sim.set_initial_pressure(400.0);
    sim.set_initial_saturation(0.2);
    sim.set_initial_gas_saturation(0.8);
    sim.set_initial_rs(0.0);
    sim.set_rock_properties(rock_compressibility, 0.0, 1.0, 1.0)
        .unwrap();
    sim.add_well(0, 0, 0, 20.0, 0.1, 0.0, false).unwrap();
    sim
}

/// Compaction must not depend on how the depletion was stepped.
///
/// A general invariant rather than a regression guard: the pre-2026-08-01
/// previous-timestep pore-volume reference passes this, because its error
/// telescopes to first order in the step size even though the total it
/// telescopes to is wrong. The test that pins that defect is
/// `physics_depletion_gas_compaction_matches_material_balance` below. This one
/// is kept because step-size independence is cheap to check and is the property
/// a future timestep or accumulation change is most likely to break.
#[test]
fn physics_depletion_gas_compaction_is_independent_of_step_size() {
    fn produced_gas_sc(dt_days: f64, steps: usize) -> (f64, f64) {
        let mut sim = make_deep_dry_gas_tank(5e-4);
        let initial_inventory = total_gas_inventory_sc_all_cells(&sim);
        for _ in 0..steps {
            sim.step(dt_days);
        }
        assert!(
            sim.last_solver_warning.is_empty(),
            "gas depletion emitted solver warning: {}",
            sim.last_solver_warning
        );
        (
            initial_inventory - total_gas_inventory_sc_all_cells(&sim),
            sim.pressure[0],
        )
    }

    let (coarse, coarse_pressure) = produced_gas_sc(40.0, 20);
    let (fine, fine_pressure) = produced_gas_sc(2.0, 400);

    assert!(
        (coarse_pressure - fine_pressure).abs() < 1.0,
        "runs did not reach the same pressure: coarse={coarse_pressure:.4}, fine={fine_pressure:.4}"
    );

    let relative_gap = (fine - coarse).abs() / coarse.max(1e-9);
    assert!(
        relative_gap < 0.02,
        "compaction depends on step size: coarse={coarse:.1} Sm3 over 20 steps, \
         fine={fine:.1} Sm3 over 400 steps, gap={:.2}%",
        relative_gap * 100.0
    );
}

/// Produced gas must match a dry-gas material balance worked by hand.
///
/// The tank is closed, the water immobile and the gas law ideal, so the answer
/// is a closed form: what is left at abandonment is the *compacted* pore volume,
/// less the connate water it still holds, at the abandonment gas density. The
/// compaction term is therefore worth the compacted volume times `1/B_g` at
/// abandonment — not at the pressures the compaction passed through, which in
/// this tank are up to twenty times denser and were what the pre-2026-08-01
/// previous-timestep reference charged it at.
#[test]
fn physics_depletion_gas_compaction_matches_material_balance() {
    const PORE_VOLUME_M3: f64 = 300.0 * 300.0 * 20.0 * 0.15;
    const INITIAL_PRESSURE_BAR: f64 = 400.0;
    const C_W: f64 = 4e-5;

    /// `G_p = G - PV(p) * S_g(p) / B_g(p)`, with `B_g = 4/p`.
    fn material_balance_produced_sc(rock_compressibility: f64, abandonment_bar: f64) -> f64 {
        let drawdown = INITIAL_PRESSURE_BAR - abandonment_bar;
        let gas_in_place = PORE_VOLUME_M3 * 0.8 * (INITIAL_PRESSURE_BAR / 4.0);
        let pore_volume = PORE_VOLUME_M3 * f64::exp(-rock_compressibility * drawdown);
        let water_volume = PORE_VOLUME_M3 * 0.2 * (1.0 + C_W * drawdown);
        let remaining = (pore_volume - water_volume) * (abandonment_bar / 4.0);
        gas_in_place - remaining
    }

    fn run(rock_compressibility: f64) -> (f64, f64) {
        let mut sim = make_deep_dry_gas_tank(rock_compressibility);
        let initial_inventory = total_gas_inventory_sc_all_cells(&sim);
        for _ in 0..400 {
            sim.step(2.0);
        }
        assert!(
            sim.last_solver_warning.is_empty(),
            "gas depletion emitted solver warning: {}",
            sim.last_solver_warning
        );
        // Reported inventory is against the *reference* pore volume, so the
        // produced total is read from the wells rather than differenced from it.
        let mut produced = 0.0;
        let mut previous_time_days = 0.0;
        for point in &sim.rate_history {
            produced += point.total_production_gas * (point.time - previous_time_days);
            previous_time_days = point.time;
        }
        let _ = initial_inventory;
        (produced, sim.pressure[0])
    }

    for rock_compressibility in [0.0, 5e-4] {
        let (produced, abandonment_bar) = run(rock_compressibility);
        let expected = material_balance_produced_sc(rock_compressibility, abandonment_bar);
        let relative_error = (produced - expected).abs() / expected;
        assert!(
            relative_error < 0.03,
            "c_f = {rock_compressibility}: produced {produced:.1} Sm3 against a material balance \
             of {expected:.1} Sm3 at {abandonment_bar:.2} bar ({:.2}% out)",
            relative_error * 100.0
        );
    }
}
