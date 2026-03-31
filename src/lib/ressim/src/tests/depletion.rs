use super::*;

const DEP_PSS_LENGTH_M: f64 = 420.0;
const DEP_PSS_WIDTH_M: f64 = 420.0;
const DEP_PSS_HEIGHT_M: f64 = 10.0;
const DEP_PSS_POROSITY: f64 = 0.2;
const DEP_PSS_PERM_MD: f64 = 50.0;
const DEP_PSS_WELL_RADIUS_M: f64 = 0.1;
const DEP_PSS_INITIAL_PRESSURE_BAR: f64 = 300.0;
const DEP_PSS_PRODUCER_BHP_BAR: f64 = 100.0;
const DEP_PSS_INITIAL_SW: f64 = 0.1;
const DEP_PSS_SWC: f64 = 0.1;
const DEP_PSS_SOR: f64 = 0.1;
const DEP_PSS_NO: f64 = 2.0;
const DEP_PSS_MU_O_CP: f64 = 1.0;
const DEP_PSS_C_O_BAR_INV: f64 = 1e-5;
const DEP_PSS_C_W_BAR_INV: f64 = 3e-6;
const DEP_PSS_C_ROCK_BAR_INV: f64 = 1e-6;
const DEP_PSS_WELL_SKIN: f64 = 0.0;
const DIETZ_CA_SQUARE_CENTER: f64 = 30.8828;
const DIETZ_CA_SQUARE_CORNER: f64 = 0.5598;
const EULER_GAMMA: f64 = 0.577_215_664_9;
const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;

#[derive(Clone, Debug)]
struct DepletionSnapshot {
    time_days: f64,
    oil_rate_sc_day: f64,
    cumulative_oil_sc: f64,
    avg_pressure_bar: f64,
    total_injection_sc_day: f64,
}

fn make_dep_pss_like_sim(
    dt_days: f64,
    steps: usize,
    producer_i: usize,
    producer_j: usize,
) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(21, 21, 1, DEP_PSS_POROSITY);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(20.0, 20.0, vec![DEP_PSS_HEIGHT_M])
        .unwrap();
    sim.set_fluid_properties(DEP_PSS_MU_O_CP, 0.5).unwrap();
    sim.set_fluid_compressibilities(DEP_PSS_C_O_BAR_INV, DEP_PSS_C_W_BAR_INV)
        .unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_rock_properties(DEP_PSS_C_ROCK_BAR_INV, DEP_PSS_POROSITY, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(DEP_PSS_INITIAL_PRESSURE_BAR);
    sim.set_initial_saturation(DEP_PSS_INITIAL_SW);
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_permeability_per_layer(vec![DEP_PSS_PERM_MD], vec![DEP_PSS_PERM_MD], vec![5.0])
        .unwrap();
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.injector_enabled = false;
    sim.add_well(
        producer_i,
        producer_j,
        0,
        DEP_PSS_PRODUCER_BHP_BAR,
        DEP_PSS_WELL_RADIUS_M,
        DEP_PSS_WELL_SKIN,
        false,
    )
    .unwrap();

    for _ in 0..steps {
        sim.step(dt_days);
        assert!(
            sim.last_solver_warning.is_empty(),
            "depletion case emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    sim
}

fn collect_depletion_snapshots(sim: &ReservoirSimulator) -> Vec<DepletionSnapshot> {
    let mut cumulative_oil_sc = 0.0;
    let mut previous_time_days = 0.0;

    sim.rate_history
        .iter()
        .map(|point| {
            let dt_days = point.time - previous_time_days;
            previous_time_days = point.time;
            cumulative_oil_sc += point.total_production_oil * dt_days;
            DepletionSnapshot {
                time_days: point.time,
                oil_rate_sc_day: point.total_production_oil,
                cumulative_oil_sc,
                avg_pressure_bar: point.avg_reservoir_pressure,
                total_injection_sc_day: point.total_injection,
            }
        })
        .collect()
}

fn kro_at_initial_sw() -> f64 {
    let mobile_range = (1.0 - DEP_PSS_SWC - DEP_PSS_SOR).max(1e-9);
    let effective_sw = ((DEP_PSS_INITIAL_SW - DEP_PSS_SWC) / mobile_range).clamp(0.0, 1.0);
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
    let pore_volume_m3 = DEP_PSS_LENGTH_M * DEP_PSS_WIDTH_M * DEP_PSS_HEIGHT_M * DEP_PSS_POROSITY;
    let drainage_area_m2 = DEP_PSS_LENGTH_M * DEP_PSS_WIDTH_M;
    let shape_factor = dietz_shape_factor(producer_i, producer_j);
    let kro = kro_at_initial_sw();
    let denominator = 0.5
        * ((4.0 * drainage_area_m2)
            / (shape_factor
                * (2.0 * EULER_GAMMA).exp()
                * DEP_PSS_WELL_RADIUS_M
                * DEP_PSS_WELL_RADIUS_M))
            .ln();
    let productivity_index = (DARCY_METRIC_FACTOR
        * 2.0
        * std::f64::consts::PI
        * DEP_PSS_PERM_MD
        * DEP_PSS_HEIGHT_M
        * (kro / DEP_PSS_MU_O_CP))
        / (denominator + DEP_PSS_WELL_SKIN).max(1e-9);
    let total_compressibility = (1.0 - DEP_PSS_INITIAL_SW) * DEP_PSS_C_O_BAR_INV
        + DEP_PSS_INITIAL_SW * DEP_PSS_C_W_BAR_INV
        + DEP_PSS_C_ROCK_BAR_INV;
    let tau_days = (pore_volume_m3 * total_compressibility) / productivity_index.max(1e-12);
    let q0 = productivity_index * (DEP_PSS_INITIAL_PRESSURE_BAR - DEP_PSS_PRODUCER_BHP_BAR);
    let oil_rate_sc_day = q0 * (-time_days / tau_days.max(1e-9)).exp();
    let avg_pressure_bar =
        DEP_PSS_PRODUCER_BHP_BAR + oil_rate_sc_day / productivity_index.max(1e-12);
    (oil_rate_sc_day, avg_pressure_bar)
}

#[test]
fn dep_pss_fim_closed_system_depletion_invariants_hold() {
    let sim = make_dep_pss_like_sim(0.1, 8, 10, 10);
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
#[ignore = "explicit refinement probe: dep_pss FIM should converge under timestep refinement without relying on IMPES"]
fn dep_pss_fim_timestep_refinement_is_locally_stable() {
    let coarse = make_dep_pss_like_sim(0.1, 8, 10, 10);
    let fine = make_dep_pss_like_sim(0.05, 16, 10, 10);

    let coarse_last = coarse
        .rate_history
        .last()
        .expect("coarse depletion case should record history");
    let fine_last = fine
        .rate_history
        .last()
        .expect("fine depletion case should record history");

    let coarse_cumulative_oil: f64 = collect_depletion_snapshots(&coarse)
        .last()
        .expect("coarse depletion snapshots should exist")
        .cumulative_oil_sc;
    let fine_cumulative_oil: f64 = collect_depletion_snapshots(&fine)
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
#[ignore = "explicit analytical probe: late-time dep_pss FIM should approach Dietz PSS decline without relying on IMPES"]
fn dep_pss_fim_late_time_matches_dietz_reference_smoke() {
    let sim = make_dep_pss_like_sim(0.2, 40, 10, 10);
    let snapshots = collect_depletion_snapshots(&sim);
    let late_time: Vec<_> = snapshots
        .iter()
        .filter(|snapshot| snapshot.time_days >= 5.0)
        .collect();

    assert!(
        !late_time.is_empty(),
        "late-time depletion window should not be empty"
    );

    for snapshot in late_time {
        let (reference_rate, reference_pressure) = dietz_pss_reference(snapshot.time_days, 10, 10);
        let rate_rel_diff =
            ((snapshot.oil_rate_sc_day - reference_rate) / reference_rate.max(1e-12)).abs();
        let pressure_rel_diff = ((snapshot.avg_pressure_bar - reference_pressure)
            / reference_pressure.max(1e-12))
        .abs();

        assert!(
            rate_rel_diff <= 0.10,
            "dep_pss late-time Dietz oil-rate drift too large at t={:.2} d: FIM={:.6}, ref={:.6}, rel_diff={:.4}",
            snapshot.time_days,
            snapshot.oil_rate_sc_day,
            reference_rate,
            rate_rel_diff,
        );
        assert!(
            pressure_rel_diff <= 0.03,
            "dep_pss late-time Dietz pressure drift too large at t={:.2} d: FIM={:.6}, ref={:.6}, rel_diff={:.4}",
            snapshot.time_days,
            snapshot.avg_pressure_bar,
            reference_pressure,
            pressure_rel_diff,
        );
    }
}
