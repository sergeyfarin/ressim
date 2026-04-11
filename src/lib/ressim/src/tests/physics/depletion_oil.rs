const DEP_PSS_SWC: f64 = 0.1;
const DEP_PSS_SOR: f64 = 0.1;
const DEP_PSS_NO: f64 = 2.0;
const DIETZ_CA_SQUARE_CENTER: f64 = 30.8828;
const DIETZ_CA_SQUARE_CORNER: f64 = 0.5598;
const EULER_GAMMA: f64 = 0.577_215_664_9;
const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;

fn kro_at_initial_sw() -> f64 {
    let mobile_range = (1.0 - DEP_PSS_SWC - DEP_PSS_SOR).max(1e-9);
    let effective_sw =
        ((super::fixtures::DEP_PSS_INITIAL_SW - DEP_PSS_SWC) / mobile_range).clamp(0.0, 1.0);
    (1.0 - effective_sw).powf(DEP_PSS_NO)
}

fn dietz_shape_factor(producer_i: usize, producer_j: usize) -> f64 {
    let cx = 10.0;
    let cy = 10.0;
    let dx = ((producer_i as f64 - cx).abs() / cx).clamp(0.0, 1.0);
    let dy = ((producer_j as f64 - cy).abs() / cy).clamp(0.0, 1.0);
    let d = dx.max(dy);
    (DIETZ_CA_SQUARE_CENTER.ln() * (1.0 - d) + DIETZ_CA_SQUARE_CORNER.ln() * d).exp()
}

fn dietz_pss_reference(time_days: f64, producer_i: usize, producer_j: usize) -> (f64, f64) {
    let pore_volume_m3 = super::fixtures::DEP_PSS_LENGTH_M
        * super::fixtures::DEP_PSS_WIDTH_M
        * super::fixtures::DEP_PSS_HEIGHT_M
        * super::fixtures::DEP_PSS_POROSITY;
    let drainage_area_m2 = super::fixtures::DEP_PSS_LENGTH_M * super::fixtures::DEP_PSS_WIDTH_M;
    let shape_factor = dietz_shape_factor(producer_i, producer_j);
    let kro = kro_at_initial_sw();
    let denominator = 0.5
        * ((4.0 * drainage_area_m2)
            / (shape_factor
                * (2.0 * EULER_GAMMA).exp()
                * super::fixtures::DEP_PSS_WELL_RADIUS_M
                * super::fixtures::DEP_PSS_WELL_RADIUS_M))
            .ln();
    let productivity_index = (DARCY_METRIC_FACTOR
        * 2.0
        * std::f64::consts::PI
        * super::fixtures::DEP_PSS_PERM_MD
        * super::fixtures::DEP_PSS_HEIGHT_M
        * (kro / super::fixtures::DEP_PSS_MU_O_CP))
        / (denominator + super::fixtures::DEP_PSS_WELL_SKIN).max(1e-9);
    let total_compressibility = (1.0 - super::fixtures::DEP_PSS_INITIAL_SW)
        * super::fixtures::DEP_PSS_C_O_BAR_INV
        + super::fixtures::DEP_PSS_INITIAL_SW * super::fixtures::DEP_PSS_C_W_BAR_INV
        + super::fixtures::DEP_PSS_C_ROCK_BAR_INV;
    let tau_days = (pore_volume_m3 * total_compressibility) / productivity_index.max(1e-12);
    let q0 = productivity_index
        * (super::fixtures::DEP_PSS_INITIAL_PRESSURE_BAR
            - super::fixtures::DEP_PSS_PRODUCER_BHP_BAR);
    let oil_rate_sc_day = q0 * (-time_days / tau_days.max(1e-9)).exp();
    let avg_pressure_bar =
        super::fixtures::DEP_PSS_PRODUCER_BHP_BAR + oil_rate_sc_day / productivity_index.max(1e-12);
    (oil_rate_sc_day, avg_pressure_bar)
}

use super::fixtures::{
    DEP_PSS_INITIAL_PRESSURE_BAR, collect_depletion_snapshots,
    make_closed_depletion_single_cell_sim, make_closed_depletion_single_cell_sim_with_storage,
    make_dep_pss_like_sim, total_component_inventory_sc_all_cells,
};

fn cumulative_reservoir_withdrawal_and_pressure_work_proxy(
    sim: &crate::ReservoirSimulator,
    producer_bhp_bar: f64,
) -> (f64, f64) {
    let mut cumulative_withdrawal_rm3 = 0.0;
    let mut pressure_work_proxy = 0.0;
    let mut previous_time_days = 0.0;

    for point in &sim.rate_history {
        let dt_days = point.time - previous_time_days;
        previous_time_days = point.time;
        cumulative_withdrawal_rm3 += point.total_production_liquid_reservoir.max(0.0) * dt_days;
        pressure_work_proxy += point.total_production_liquid_reservoir.max(0.0)
            * (point.avg_reservoir_pressure - producer_bhp_bar).max(0.0)
            * dt_days;
    }

    (cumulative_withdrawal_rm3, pressure_work_proxy)
}

#[test]
fn physics_depletion_oil_closed_system_monotone() {
    let sim = make_dep_pss_like_sim(0.1, 8);
    let snapshots = collect_depletion_snapshots(&sim);

    assert!(!snapshots.is_empty());

    let mut previous_pressure = DEP_PSS_INITIAL_PRESSURE_BAR;
    let mut previous_rate = f64::INFINITY;
    let mut previous_cumulative_oil = 0.0;
    for snapshot in &snapshots {
        assert!(snapshot.total_injection_sc_day.abs() <= 1e-12);
        assert!(snapshot.avg_pressure_bar <= previous_pressure + 1e-9);
        assert!(snapshot.oil_rate_sc_day <= previous_rate + 1e-9);
        assert!(snapshot.cumulative_oil_sc >= previous_cumulative_oil - 1e-9);

        previous_pressure = snapshot.avg_pressure_bar;
        previous_rate = snapshot.oil_rate_sc_day;
        previous_cumulative_oil = snapshot.cumulative_oil_sc;
    }
}

#[test]
fn physics_depletion_oil_public_reporting_contract_holds_on_both_solvers() {
    fn run_case(fim_enabled: bool) -> (f64, f64, f64, f64, f64, f64, usize) {
        let mut sim = make_closed_depletion_single_cell_sim();
        sim.set_fim_enabled(fim_enabled);

        let initial_oil_inventory = total_component_inventory_sc_all_cells(&sim).oil_sc;
        let mut previous_pressure = sim.pressure[0];
        let mut previous_cumulative_oil = 0.0;
        let mut previous_time_days = 0.0;
        let mut cumulative_oil_sc = 0.0;

        for _ in 0..8 {
            sim.step(0.05);
            assert!(
                sim.last_solver_warning.is_empty(),
                "two-solver oil depletion public-contract case emitted solver warning for fim_enabled={}: {}",
                fim_enabled,
                sim.last_solver_warning
            );

            let point = sim
                .rate_history
                .last()
                .expect("two-solver oil depletion public-contract case should record history");
            let dt_days = point.time - previous_time_days;
            previous_time_days = point.time;
            cumulative_oil_sc += point.total_production_oil.max(0.0) * dt_days;

            assert!(point.total_injection.abs() <= 1e-12);
            assert!(point.total_production_oil > 0.0);
            assert!(point.avg_reservoir_pressure.is_finite());
            assert!(point.material_balance_error_oil_m3.is_finite());
            assert!(
                point.avg_reservoir_pressure <= previous_pressure + 1e-9,
                "oil depletion pressure should not increase for fim_enabled={}: prev={:.6}, now={:.6}",
                fim_enabled,
                previous_pressure,
                point.avg_reservoir_pressure
            );
            assert!(
                cumulative_oil_sc >= previous_cumulative_oil - 1e-12,
                "oil depletion cumulative production should not decrease for fim_enabled={}: prev={:.6}, now={:.6}",
                fim_enabled,
                previous_cumulative_oil,
                cumulative_oil_sc
            );

            previous_pressure = point.avg_reservoir_pressure;
            previous_cumulative_oil = cumulative_oil_sc;
        }

        let latest = sim
            .rate_history
            .last()
            .expect("two-solver oil depletion public-contract case should record history");
        let final_oil_inventory = total_component_inventory_sc_all_cells(&sim).oil_sc;

        (
            initial_oil_inventory - final_oil_inventory,
            cumulative_oil_sc,
            latest.material_balance_error_oil_m3,
            latest.producer_bhp_limited_fraction,
            latest.injector_bhp_limited_fraction,
            latest.time,
            sim.rate_history.len(),
        )
    }

    for (fim_enabled, metrics) in [(false, run_case(false)), (true, run_case(true))] {
        assert!(
            metrics.0 > 0.0,
            "expected oil inventory depletion for fim_enabled={}",
            fim_enabled
        );
        assert!(
            metrics.1 > 0.0,
            "expected cumulative oil production for fim_enabled={}",
            fim_enabled
        );
        assert!(metrics.2.is_finite());
        assert!((0.0..=1.0).contains(&metrics.3));
        assert!((0.0..=1.0).contains(&metrics.4));
        assert!((metrics.5 - 0.4).abs() <= 1e-9);
        assert!(
            metrics.6 > 0,
            "expected rate history for fim_enabled={}",
            fim_enabled
        );
    }
}

#[test]
fn physics_depletion_oil_higher_oil_compressibility_cushions_pressure_drop() {
    let mut low_storage = make_closed_depletion_single_cell_sim_with_storage(0.0, 0.0, 0.0, 100.0);
    let mut high_storage =
        make_closed_depletion_single_cell_sim_with_storage(5e-5, 3e-6, 5e-6, 100.0);

    low_storage.step(0.05);
    high_storage.step(0.05);

    assert!(
        low_storage.last_solver_warning.is_empty(),
        "low-storage depletion emitted solver warning: {}",
        low_storage.last_solver_warning
    );
    assert!(
        high_storage.last_solver_warning.is_empty(),
        "high-storage depletion emitted solver warning: {}",
        high_storage.last_solver_warning
    );

    let low_pressure = low_storage
        .rate_history
        .last()
        .unwrap()
        .avg_reservoir_pressure;
    let high_pressure = high_storage
        .rate_history
        .last()
        .unwrap()
        .avg_reservoir_pressure;

    assert!(
        high_pressure > low_pressure + 1e-3,
        "higher compressive storage should cushion pressure drop: low-storage p={:.6}, high-storage p={:.6}",
        low_pressure,
        high_pressure
    );
}

#[test]
fn physics_depletion_oil_stronger_drawdown_increases_pressure_work_proxy() {
    let mut mild_drawdown =
        make_closed_depletion_single_cell_sim_with_storage(1e-5, 3e-6, 1e-6, 180.0);
    let mut strong_drawdown =
        make_closed_depletion_single_cell_sim_with_storage(1e-5, 3e-6, 1e-6, 80.0);

    mild_drawdown.step(0.05);
    strong_drawdown.step(0.05);

    assert!(
        mild_drawdown.last_solver_warning.is_empty(),
        "mild-drawdown depletion emitted solver warning: {}",
        mild_drawdown.last_solver_warning
    );
    assert!(
        strong_drawdown.last_solver_warning.is_empty(),
        "strong-drawdown depletion emitted solver warning: {}",
        strong_drawdown.last_solver_warning
    );

    let mild_latest = mild_drawdown.rate_history.last().unwrap();
    let strong_latest = strong_drawdown.rate_history.last().unwrap();
    let (mild_withdrawal, mild_work) =
        cumulative_reservoir_withdrawal_and_pressure_work_proxy(&mild_drawdown, 180.0);
    let (strong_withdrawal, strong_work) =
        cumulative_reservoir_withdrawal_and_pressure_work_proxy(&strong_drawdown, 80.0);

    assert!(
        strong_latest.total_production_oil > mild_latest.total_production_oil,
        "stronger drawdown should increase oil production rate: mild={:.6}, strong={:.6}",
        mild_latest.total_production_oil,
        strong_latest.total_production_oil
    );
    assert!(
        strong_withdrawal > mild_withdrawal,
        "stronger drawdown should increase cumulative reservoir withdrawal: mild={:.6}, strong={:.6}",
        mild_withdrawal,
        strong_withdrawal
    );
    assert!(
        strong_work > mild_work,
        "stronger drawdown should increase the pressure-work proxy: mild={:.6}, strong={:.6}",
        mild_work,
        strong_work
    );
    assert!(
        strong_latest.avg_reservoir_pressure <= mild_latest.avg_reservoir_pressure + 1e-9,
        "stronger drawdown should not leave a higher reservoir pressure: mild={:.6}, strong={:.6}",
        mild_latest.avg_reservoir_pressure,
        strong_latest.avg_reservoir_pressure
    );
}

#[test]
fn physics_depletion_oil_rock_compressibility_adds_storage_response() {
    let mut stiff_rock = make_closed_depletion_single_cell_sim_with_storage(1e-5, 3e-6, 0.0, 100.0);
    let mut compressible_rock =
        make_closed_depletion_single_cell_sim_with_storage(1e-5, 3e-6, 5e-5, 100.0);

    stiff_rock.step(0.05);
    compressible_rock.step(0.05);

    assert!(
        stiff_rock.last_solver_warning.is_empty(),
        "stiff-rock depletion emitted solver warning: {}",
        stiff_rock.last_solver_warning
    );
    assert!(
        compressible_rock.last_solver_warning.is_empty(),
        "compressible-rock depletion emitted solver warning: {}",
        compressible_rock.last_solver_warning
    );

    let stiff_pressure = stiff_rock
        .rate_history
        .last()
        .unwrap()
        .avg_reservoir_pressure;
    let compressible_pressure = compressible_rock
        .rate_history
        .last()
        .unwrap()
        .avg_reservoir_pressure;

    assert!(
        compressible_pressure > stiff_pressure + 1e-3,
        "higher rock compressibility should cushion pressure drop: stiff={:.6}, compressible={:.6}",
        stiff_pressure,
        compressible_pressure
    );
}

#[test]
#[ignore = "explicit refinement probe: dep_pss FIM should converge under timestep refinement without relying on IMPES"]
fn physics_depletion_oil_dep_pss_timestep_refinement_is_locally_stable() {
    let coarse = make_dep_pss_like_sim(0.1, 8);
    let fine = make_dep_pss_like_sim(0.05, 16);

    let coarse_last = coarse
        .rate_history
        .last()
        .expect("coarse depletion case should record history");
    let fine_last = fine
        .rate_history
        .last()
        .expect("fine depletion case should record history");

    let coarse_cumulative_oil = collect_depletion_snapshots(&coarse)
        .last()
        .expect("coarse depletion snapshots should exist")
        .cumulative_oil_sc;
    let fine_cumulative_oil = collect_depletion_snapshots(&fine)
        .last()
        .expect("fine depletion snapshots should exist")
        .cumulative_oil_sc;

    let oil_rate_rel_diff = ((coarse_last.total_production_oil - fine_last.total_production_oil)
        / fine_last.total_production_oil.max(1e-12))
    .abs();
    let cumulative_oil_rel_diff =
        ((coarse_cumulative_oil - fine_cumulative_oil) / fine_cumulative_oil.max(1e-12)).abs();
    let avg_pressure_rel_diff = ((coarse_last.avg_reservoir_pressure
        - fine_last.avg_reservoir_pressure)
        / fine_last.avg_reservoir_pressure.max(1e-12))
    .abs();

    assert!(
        oil_rate_rel_diff <= 0.05,
        "dep_pss timestep refinement oil-rate drift too large: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_last.total_production_oil,
        fine_last.total_production_oil,
        oil_rate_rel_diff,
    );
    assert!(
        cumulative_oil_rel_diff <= 0.03,
        "dep_pss timestep refinement cumulative-oil drift too large: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_cumulative_oil,
        fine_cumulative_oil,
        cumulative_oil_rel_diff,
    );
    assert!(
        avg_pressure_rel_diff <= 0.01,
        "dep_pss timestep refinement avg-pressure drift too large: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_last.avg_reservoir_pressure,
        fine_last.avg_reservoir_pressure,
        avg_pressure_rel_diff,
    );
}

#[test]
#[ignore = "diagnostic analytical probe: characterize late-time dep_pss vs Dietz drift while the model-alignment gap remains open"]
fn physics_depletion_oil_dep_pss_late_time_matches_dietz_reference_smoke() {
    let sim = make_dep_pss_like_sim(0.2, 40);
    let snapshots = collect_depletion_snapshots(&sim);
    let late_time: Vec<_> = snapshots
        .iter()
        .filter(|snapshot| snapshot.time_days >= 5.0)
        .collect();

    assert!(
        !late_time.is_empty(),
        "late-time depletion window should not be empty"
    );

    let mut max_rate_rel_diff = 0.0_f64;
    let mut max_pressure_rel_diff = 0.0_f64;

    for snapshot in late_time {
        let (reference_rate, reference_pressure) = dietz_pss_reference(snapshot.time_days, 10, 10);
        let rate_rel_diff =
            ((snapshot.oil_rate_sc_day - reference_rate) / reference_rate.max(1e-12)).abs();
        let pressure_rel_diff = ((snapshot.avg_pressure_bar - reference_pressure)
            / reference_pressure.max(1e-12))
        .abs();

        max_rate_rel_diff = max_rate_rel_diff.max(rate_rel_diff);
        max_pressure_rel_diff = max_pressure_rel_diff.max(pressure_rel_diff);
    }

    assert!(
        max_rate_rel_diff <= 2.00,
        "dep_pss late-time Dietz oil-rate drift exceeded the current diagnostic envelope: max_rel_diff={:.4}",
        max_rate_rel_diff,
    );
    assert!(
        max_pressure_rel_diff <= 0.12,
        "dep_pss late-time Dietz pressure drift exceeded the current diagnostic envelope: max_rel_diff={:.4}",
        max_pressure_rel_diff,
    );
}
