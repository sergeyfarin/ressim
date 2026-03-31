use super::*;

use crate::fim::assembly::{FimAssemblyOptions, assemble_fim_system};
use crate::fim::newton::{FimNewtonOptions, run_fim_timestep};
use crate::fim::state::FimState;
use crate::fim::wells::build_well_topology;

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

#[derive(Clone, Debug)]
struct DepletionStepDiagnostics {
    time_days: f64,
    oil_rate_sc_day: f64,
    avg_pressure_bar: f64,
    avg_water_saturation: f64,
    producer_cell_pressure_bar: f64,
    producer_cell_sw: f64,
    producer_bhp_bar: f64,
    producer_perf_rate_m3_day: f64,
    accepted_substep_count: usize,
    accepted_substep_dts: Vec<f64>,
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

fn parse_accepted_substep_dts(trace: &str) -> Vec<f64> {
    trace
        .lines()
        .filter_map(|line| {
            let marker = ": ACCEPTED dt=";
            let start = line.find(marker)? + marker.len();
            let rest = &line[start..];
            let value = rest.split_whitespace().next()?;
            value.parse::<f64>().ok()
        })
        .collect()
}

fn run_dep_pss_like_sim_with_step_diagnostics(
    dt_days: f64,
    steps: usize,
    producer_i: usize,
    producer_j: usize,
) -> (ReservoirSimulator, Vec<DepletionStepDiagnostics>) {
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

    let mut diagnostics = Vec::with_capacity(steps);
    for _ in 0..steps {
        let trace = sim.step_with_diagnostics(dt_days);
        assert!(
            sim.last_solver_warning.is_empty(),
            "depletion case emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );

        let accepted_substep_dts = parse_accepted_substep_dts(&trace);
        let topology = build_well_topology(&sim);
        let producer_well = &topology.wells[0];
        let perf_idx = producer_well.perforation_indices[0];
        let perf = &topology.perforations[perf_idx];
        let state = FimState::from_simulator(&sim);
        let rate_point = sim
            .rate_history
            .last()
            .expect("depletion diagnostic run should record rate history");

        diagnostics.push(DepletionStepDiagnostics {
            time_days: sim.time_days,
            oil_rate_sc_day: rate_point.total_production_oil,
            avg_pressure_bar: rate_point.avg_reservoir_pressure,
            avg_water_saturation: rate_point.avg_water_saturation,
            producer_cell_pressure_bar: sim.pressure[perf.cell_index],
            producer_cell_sw: sim.sat_water[perf.cell_index],
            producer_bhp_bar: state.well_bhp[0],
            producer_perf_rate_m3_day: state.perforation_rates_m3_day[perf_idx],
            accepted_substep_count: accepted_substep_dts.len(),
            accepted_substep_dts,
        });
    }

    (sim, diagnostics)
}

fn make_closed_depletion_single_cell_sim() -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(1, 1, 1, DEP_PSS_POROSITY);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(DEP_PSS_LENGTH_M, DEP_PSS_WIDTH_M, vec![DEP_PSS_HEIGHT_M])
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
        0,
        0,
        0,
        DEP_PSS_PRODUCER_BHP_BAR,
        DEP_PSS_WELL_RADIUS_M,
        DEP_PSS_WELL_SKIN,
        false,
    )
    .unwrap();
    sim
}

#[derive(Clone, Debug)]
struct LocalNewtonDiagnostics {
    pressure_bar: f64,
    oil_rate_sc_day: f64,
    residual_inf_norm: f64,
    material_balance_inf_norm: f64,
    oil_residual_abs_sc: f64,
    oil_residual_scale_sc: f64,
    newton_iterations: usize,
}

fn run_single_cell_local_newton(dt_days: f64, options: FimNewtonOptions) -> LocalNewtonDiagnostics {
    let mut sim = make_closed_depletion_single_cell_sim();
    let previous_state = FimState::from_simulator(&sim);
    let report = run_fim_timestep(&mut sim, &previous_state, &previous_state, dt_days, &options);
    assert!(report.converged, "single-cell local Newton diagnostic should converge");

    let topology = build_well_topology(&sim);
    let assembly = assemble_fim_system(
        &sim,
        &previous_state,
        &report.accepted_state,
        &FimAssemblyOptions {
            dt_days,
            include_wells: true,
            assemble_residual_only: false,
            topology: Some(&topology),
        },
    );
    let perf_idx = topology.wells[0].perforation_indices[0];
    let oil_rate_sc_day = report.accepted_state.perforation_rates_m3_day[perf_idx]
        / report.accepted_state.derive_cell(&sim, 0).bo.max(1e-9);

    LocalNewtonDiagnostics {
        pressure_bar: report.accepted_state.cells[0].pressure_bar,
        oil_rate_sc_day,
        residual_inf_norm: report.final_residual_inf_norm,
        material_balance_inf_norm: report.final_material_balance_inf_norm,
        oil_residual_abs_sc: assembly.residual[1].abs(),
        oil_residual_scale_sc: assembly.equation_scaling.oil_component[0],
        newton_iterations: report.newton_iterations,
    }
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
fn dep_pss_fim_single_cell_local_newton_leaves_small_absolute_oil_residual() {
    let diagnostics = run_single_cell_local_newton(0.1, FimNewtonOptions::default());

    assert!(diagnostics.residual_inf_norm <= 1e-5);
    assert!(diagnostics.material_balance_inf_norm <= 1e-5);
    assert!(
        diagnostics.oil_residual_abs_sc <= 0.1,
        "single-cell local depletion accepted too much absolute oil imbalance: oil_abs={:.6} scale={:.6}",
        diagnostics.oil_residual_abs_sc,
        diagnostics.oil_residual_scale_sc,
    );
}

#[test]
fn dep_pss_fim_single_cell_depletion_is_timestep_stable() {
    let mut coarse = make_closed_depletion_single_cell_sim();
    coarse.step(0.1);
    assert!(
        coarse.last_solver_warning.is_empty(),
        "single-cell coarse depletion emitted solver warning: {}",
        coarse.last_solver_warning
    );
    let coarse_point = coarse
        .rate_history
        .last()
        .expect("single-cell coarse depletion should record history");

    let mut fine = make_closed_depletion_single_cell_sim();
    fine.step(0.05);
    fine.step(0.05);
    assert!(
        fine.last_solver_warning.is_empty(),
        "single-cell fine depletion emitted solver warning: {}",
        fine.last_solver_warning
    );
    let fine_point = fine
        .rate_history
        .last()
        .expect("single-cell fine depletion should record history");

    let oil_rate_rel_diff = ((coarse_point.total_production_oil - fine_point.total_production_oil)
        / fine_point.total_production_oil.max(1e-12))
    .abs();
    let avg_pressure_rel_diff = ((coarse_point.avg_reservoir_pressure
        - fine_point.avg_reservoir_pressure)
        / fine_point.avg_reservoir_pressure.max(1e-12))
    .abs();

    assert!(
        oil_rate_rel_diff <= 0.01,
        "single-cell depletion timestep oil-rate drift too large: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_point.total_production_oil,
        fine_point.total_production_oil,
        oil_rate_rel_diff,
    );
    assert!(
        avg_pressure_rel_diff <= 0.005,
        "single-cell depletion timestep pressure drift too large: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_point.avg_reservoir_pressure,
        fine_point.avg_reservoir_pressure,
        avg_pressure_rel_diff,
    );
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

#[test]
#[ignore = "diagnostic probe: compare coarse and fine dep_pss FIM step histories"]
fn dep_pss_fim_refinement_diagnostics_trace_rate_loss() {
    let (_coarse_sim, coarse_steps) = run_dep_pss_like_sim_with_step_diagnostics(0.1, 8, 10, 10);
    let (_fine_sim, fine_steps) = run_dep_pss_like_sim_with_step_diagnostics(0.05, 16, 10, 10);

    eprintln!(
        "time coarse_rate fine_rate coarse_avg_p fine_avg_p coarse_avg_sw fine_avg_sw coarse_cell_p fine_cell_p coarse_cell_sw fine_cell_sw coarse_perf_q fine_perf_q coarse_substeps fine_substeps coarse_dts fine_dts"
    );

    for coarse_idx in 0..coarse_steps.len() {
        let fine_idx = coarse_idx * 2 + 1;
        let coarse = &coarse_steps[coarse_idx];
        let fine = &fine_steps[fine_idx];
        eprintln!(
            "{:.2} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {} {} {:?} {:?}",
            coarse.time_days,
            coarse.oil_rate_sc_day,
            fine.oil_rate_sc_day,
            coarse.avg_pressure_bar,
            fine.avg_pressure_bar,
            coarse.avg_water_saturation,
            fine.avg_water_saturation,
            coarse.producer_cell_pressure_bar,
            fine.producer_cell_pressure_bar,
            coarse.producer_cell_sw,
            fine.producer_cell_sw,
            coarse.producer_perf_rate_m3_day,
            fine.producer_perf_rate_m3_day,
            coarse.accepted_substep_count,
            fine.accepted_substep_count,
            coarse.accepted_substep_dts,
            fine.accepted_substep_dts,
        );
    }

    let coarse_last = coarse_steps.last().expect("coarse diagnostics should exist");
    let fine_last = fine_steps.last().expect("fine diagnostics should exist");
    eprintln!(
        "final coarse_bhp={:.6} fine_bhp={:.6}",
        coarse_last.producer_bhp_bar,
        fine_last.producer_bhp_bar,
    );
}

#[test]
#[ignore = "diagnostic probe: isolate local cell vs spatial flux contribution in depletion refinement drift"]
fn dep_pss_fim_single_cell_refinement_diagnostics() {
    let mut coarse = make_closed_depletion_single_cell_sim();
    let coarse_trace = coarse.step_with_diagnostics(0.1);
    let coarse_point = coarse
        .rate_history
        .last()
        .expect("single-cell coarse run should record history");

    let mut fine = make_closed_depletion_single_cell_sim();
    let fine_trace_a = fine.step_with_diagnostics(0.05);
    let fine_trace_b = fine.step_with_diagnostics(0.05);
    let fine_point = fine
        .rate_history
        .last()
        .expect("single-cell fine run should record history");

    eprintln!(
        "single-cell t=0.10 coarse_rate={:.6} fine_rate={:.6} coarse_avg_p={:.6} fine_avg_p={:.6} coarse_sw={:.6} fine_sw={:.6}",
        coarse_point.total_production_oil,
        fine_point.total_production_oil,
        coarse_point.avg_reservoir_pressure,
        fine_point.avg_reservoir_pressure,
        coarse.sat_water[0],
        fine.sat_water[0],
    );
    eprintln!("single-cell coarse_trace:\n{}", coarse_trace);
    eprintln!("single-cell fine_trace_step1:\n{}", fine_trace_a);
    eprintln!("single-cell fine_trace_step2:\n{}", fine_trace_b);
}

#[test]
#[ignore = "diagnostic probe: compare loose vs tight local Newton acceptance in single-cell depletion"]
fn dep_pss_fim_single_cell_tight_newton_diagnostics() {
    let loose = run_single_cell_local_newton(0.1, FimNewtonOptions::default());
    let tight = run_single_cell_local_newton(
        0.1,
        FimNewtonOptions {
            residual_tolerance: 1e-8,
            material_balance_tolerance: 1e-8,
            max_newton_iterations: 40,
            ..FimNewtonOptions::default()
        },
    );

    eprintln!(
        "single-cell local Newton loose: p={:.6} q={:.6} res={:.3e} mb={:.3e} oil_abs={:.6} oil_scale={:.6} iters={}",
        loose.pressure_bar,
        loose.oil_rate_sc_day,
        loose.residual_inf_norm,
        loose.material_balance_inf_norm,
        loose.oil_residual_abs_sc,
        loose.oil_residual_scale_sc,
        loose.newton_iterations,
    );
    eprintln!(
        "single-cell local Newton tight: p={:.6} q={:.6} res={:.3e} mb={:.3e} oil_abs={:.6} oil_scale={:.6} iters={}",
        tight.pressure_bar,
        tight.oil_rate_sc_day,
        tight.residual_inf_norm,
        tight.material_balance_inf_norm,
        tight.oil_residual_abs_sc,
        tight.oil_residual_scale_sc,
        tight.newton_iterations,
    );
}
