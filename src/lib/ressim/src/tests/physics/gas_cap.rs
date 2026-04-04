use super::fixtures::{
    cumulative_component_production_sc, make_free_gas_cap_runtime_sim,
    run_hydrostatic_gravity_benchmark, total_component_inventory_sc_all_cells,
};
use crate::ReservoirSimulator;

#[derive(Clone, Copy)]
struct GravityColumnCase {
    name: &'static str,
    perm_md: f64,
    initial_sw: f64,
    pc_entry_bar: f64,
}

fn run_gravity_column_case(case: GravityColumnCase, gravity_enabled: bool) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(1, 1, 3, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(20.0, 20.0, vec![4.0, 6.0, 10.0])
        .unwrap();
    sim.set_permeability_per_layer(
        vec![case.perm_md, case.perm_md, case.perm_md],
        vec![case.perm_md, case.perm_md, case.perm_md],
        vec![case.perm_md, case.perm_md, case.perm_md],
    )
    .unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_initial_pressure(300.0);
    sim.set_initial_saturation(case.initial_sw);
    sim.set_capillary_params(case.pc_entry_bar, 2.0).unwrap();
    sim.set_gravity_enabled(gravity_enabled);
    sim
}

#[test]
fn physics_gas_cap_vertical_column_builds_hydrostatic_gradient() {
    let mut sim_no_g = ReservoirSimulator::new(1, 1, 2, 0.2);
    sim_no_g
        .set_permeability_random_seeded(50_000.0, 50_000.0, 42)
        .unwrap();
    sim_no_g.set_initial_pressure(300.0);
    sim_no_g.set_initial_saturation(0.9);
    sim_no_g.pc.p_entry = 0.0;
    sim_no_g.set_gravity_enabled(false);
    sim_no_g.step(2.0);

    let p_top_no_g = sim_no_g.pressure[sim_no_g.idx(0, 0, 0)];
    let p_bot_no_g = sim_no_g.pressure[sim_no_g.idx(0, 0, 1)];

    let mut sim_g = ReservoirSimulator::new(1, 1, 2, 0.2);
    sim_g
        .set_permeability_random_seeded(50_000.0, 50_000.0, 42)
        .unwrap();
    sim_g.set_initial_pressure(300.0);
    sim_g.set_initial_saturation(0.9);
    sim_g.pc.p_entry = 0.0;
    sim_g.set_gravity_enabled(true);
    sim_g.step(2.0);

    let p_top_g = sim_g.pressure[sim_g.idx(0, 0, 0)];
    let p_bot_g = sim_g.pressure[sim_g.idx(0, 0, 1)];

    assert!((p_bot_no_g - p_top_no_g).abs() < 1e-5);
    assert!(p_bot_g > p_top_g);
}

#[test]
fn physics_gas_cap_vertical_column_fim_matches_impes_hydrostatic_benchmark() {
    let impes = run_hydrostatic_gravity_benchmark(false);
    let fim = run_hydrostatic_gravity_benchmark(true);

    let gradient_rel_diff = ((fim.pressure_gradient_bar - impes.pressure_gradient_bar)
        / impes.pressure_gradient_bar.max(1e-12))
    .abs();
    let top_sw_abs_diff = (fim.top_sw_change - impes.top_sw_change).abs();

    assert!(fim.pressure_gradient_bar > 0.0);
    assert!(
        gradient_rel_diff <= 0.05,
        "gravity benchmark pressure-gradient drift too large: IMPES={:.6}, FIM={:.6}, rel_diff={:.4}",
        impes.pressure_gradient_bar,
        fim.pressure_gradient_bar,
        gradient_rel_diff,
    );
    assert!(
        top_sw_abs_diff <= 1e-4,
        "gravity benchmark top-cell Sw drift too large: IMPES={:.6}, FIM={:.6}, abs_diff={:.6}",
        impes.top_sw_change,
        fim.top_sw_change,
        top_sw_abs_diff,
    );
}

#[test]
fn physics_gas_cap_gravity_term_magnitude_matches_hydrostatic_analytical() {
    let mut sim = ReservoirSimulator::new(1, 1, 2, 0.2);
    sim.set_fim_enabled(true);
    sim.set_permeability_random_seeded(80_000.0, 80_000.0, 7)
        .unwrap();
    sim.set_initial_saturation(0.9);
    sim.pc.p_entry = 0.0;
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_gravity_enabled(true);

    let expected_dp_bar = sim.pvt.rho_w * 9.80665 * sim.dz[0] * 1e-5;
    let top_id = sim.idx(0, 0, 0);
    let bottom_id = sim.idx(0, 0, 1);
    sim.pressure[top_id] = 300.0;
    sim.pressure[bottom_id] = 300.0 + expected_dp_bar;

    sim.step(5.0);

    assert!(
        sim.last_solver_warning.is_empty(),
        "hydrostatic gravity magnitude case emitted solver warning: {}",
        sim.last_solver_warning
    );

    let measured_dp_bar = sim.pressure[bottom_id] - sim.pressure[top_id];
    assert!(
        (measured_dp_bar - expected_dp_bar).abs() < 1e-3,
        "gravity term magnitude should match hydrostatic analytical dp: expected={:.6}, measured={:.6}",
        expected_dp_bar,
        measured_dp_bar
    );
}

#[test]
fn physics_gas_cap_vertical_column_gravity_stays_quieter_than_no_gravity() {
    let initial_sw = 0.9;

    let mut sim_g = ReservoirSimulator::new(1, 1, 2, 0.2);
    sim_g
        .set_permeability_random_seeded(80_000.0, 80_000.0, 7)
        .unwrap();
    sim_g.set_initial_saturation(initial_sw);
    sim_g.pc.p_entry = 0.0;
    sim_g.set_fluid_densities(800.0, 1000.0).unwrap();
    sim_g.set_gravity_enabled(true);

    let hydro_dp_bar = sim_g.pvt.rho_w * 9.80665 * sim_g.dz[0] * 1e-5;
    let top_id_g = sim_g.idx(0, 0, 0);
    let bot_id_g = sim_g.idx(0, 0, 1);
    sim_g.pressure[top_id_g] = 300.0;
    sim_g.pressure[bot_id_g] = 300.0 + hydro_dp_bar;
    sim_g.step(5.0);
    let sw_change_top_g = (sim_g.sat_water[top_id_g] - initial_sw).abs();

    let mut sim_no_g = ReservoirSimulator::new(1, 1, 2, 0.2);
    sim_no_g
        .set_permeability_random_seeded(80_000.0, 80_000.0, 7)
        .unwrap();
    sim_no_g.set_initial_saturation(initial_sw);
    sim_no_g.pc.p_entry = 0.0;
    sim_no_g.set_fluid_densities(800.0, 1000.0).unwrap();
    sim_no_g.set_gravity_enabled(false);

    let top_id_no_g = sim_no_g.idx(0, 0, 0);
    let bot_id_no_g = sim_no_g.idx(0, 0, 1);
    sim_no_g.pressure[top_id_no_g] = 300.0;
    sim_no_g.pressure[bot_id_no_g] = 300.0 + hydro_dp_bar;
    sim_no_g.step(5.0);
    let sw_change_top_no_g = (sim_no_g.sat_water[top_id_no_g] - initial_sw).abs();

    assert!(
        sw_change_top_g <= sw_change_top_no_g + 1e-9,
        "gravity-enabled top-cell saturation drift ({}) exceeded no-gravity drift ({})",
        sw_change_top_g,
        sw_change_top_no_g
    );
}

#[test]
fn physics_gas_cap_gravity_matrix_preserves_depth_ordering_across_perm_and_capillary_ranges() {
    let cases = [
        GravityColumnCase {
            name: "no capillary high perm",
            perm_md: 80_000.0,
            initial_sw: 0.90,
            pc_entry_bar: 0.0,
        },
        GravityColumnCase {
            name: "moderate capillary",
            perm_md: 20_000.0,
            initial_sw: 0.85,
            pc_entry_bar: 2.0,
        },
        GravityColumnCase {
            name: "stronger capillary lower perm",
            perm_md: 5_000.0,
            initial_sw: 0.80,
            pc_entry_bar: 5.0,
        },
    ];

    for case in cases {
        let mut sim_g = run_gravity_column_case(case, true);
        let mut sim_no_g = run_gravity_column_case(case, false);

        sim_g.step(2.0);
        sim_no_g.step(2.0);

        assert!(
            sim_g.last_solver_warning.is_empty(),
            "{} gravity-on column emitted solver warning: {}",
            case.name,
            sim_g.last_solver_warning
        );
        assert!(
            sim_no_g.last_solver_warning.is_empty(),
            "{} gravity-off column emitted solver warning: {}",
            case.name,
            sim_no_g.last_solver_warning
        );

        for k in 1..sim_g.nz {
            let upper = sim_g.idx(0, 0, k - 1);
            let lower = sim_g.idx(0, 0, k);
            assert!(
                sim_g.pressure[lower] > sim_g.pressure[upper],
                "{} gravity-on pressure should increase with depth: p[k-1]={:.6}, p[k]={:.6}",
                case.name,
                sim_g.pressure[upper],
                sim_g.pressure[lower]
            );
            assert!(
                (sim_no_g.pressure[lower] - sim_no_g.pressure[upper]).abs() < 1e-4,
                "{} gravity-off pressure should stay nearly flat: p[k-1]={:.6}, p[k]={:.6}",
                case.name,
                sim_no_g.pressure[upper],
                sim_no_g.pressure[lower]
            );
        }
    }
}

#[test]
fn physics_gas_cap_free_gas_runtime_capillary_entry_changes_gas_cap_support() {
    let mut no_capillary = make_free_gas_cap_runtime_sim(0.0);
    let mut with_capillary = make_free_gas_cap_runtime_sim(5.0);

    let initial_no_cap = total_component_inventory_sc_all_cells(&no_capillary);
    let initial_with_cap = total_component_inventory_sc_all_cells(&with_capillary);

    for _ in 0..8 {
        no_capillary.step(0.5);
        with_capillary.step(0.5);

        assert!(
            no_capillary.last_solver_warning.is_empty(),
            "free-gas gas-cap zero-capillary case emitted solver warning at t={}: {}",
            no_capillary.time_days,
            no_capillary.last_solver_warning
        );
        assert!(
            with_capillary.last_solver_warning.is_empty(),
            "free-gas gas-cap capillary-entry case emitted solver warning at t={}: {}",
            with_capillary.time_days,
            with_capillary.last_solver_warning
        );
    }

    let top_id = no_capillary.idx(0, 0, 0);
    let bottom_id = no_capillary.idx(0, 0, 2);

    let bottom_sg_delta = (with_capillary.sat_gas[bottom_id] - no_capillary.sat_gas[bottom_id]).abs();
    let top_sg_delta = (with_capillary.sat_gas[top_id] - no_capillary.sat_gas[top_id]).abs();

    let no_cap_gas_produced = cumulative_component_production_sc(&no_capillary).gas_sc;
    let with_cap_gas_produced = cumulative_component_production_sc(&with_capillary).gas_sc;
    let produced_gas_delta = (with_cap_gas_produced - no_cap_gas_produced).abs();

    assert!(
        bottom_sg_delta > 1e-2,
        "gas-oil capillary entry should measurably change producer-layer gas arrival: no-pc Sg_bottom={:.6}, with-pc Sg_bottom={:.6}",
        no_capillary.sat_gas[bottom_id],
        with_capillary.sat_gas[bottom_id]
    );
    assert!(
        top_sg_delta > 1e-2,
        "gas-oil capillary entry should measurably change cap-layer gas retention: no-pc Sg_top={:.6}, with-pc Sg_top={:.6}",
        no_capillary.sat_gas[top_id],
        with_capillary.sat_gas[top_id]
    );
    assert!(
        produced_gas_delta > 1e-1,
        "gas-oil capillary entry should measurably change gas-cap delivery to the producer: no-pc produced gas={:.6}, with-pc produced gas={:.6}",
        no_cap_gas_produced,
        with_cap_gas_produced
    );

    let final_no_cap = total_component_inventory_sc_all_cells(&no_capillary);
    let final_with_cap = total_component_inventory_sc_all_cells(&with_capillary);
    let produced_no_cap = cumulative_component_production_sc(&no_capillary);
    let produced_with_cap = cumulative_component_production_sc(&with_capillary);

    let no_cap_water_accounted = final_no_cap.water_sc + produced_no_cap.water_sc;
    let no_cap_oil_accounted = final_no_cap.oil_sc + produced_no_cap.oil_sc;
    let no_cap_gas_accounted = final_no_cap.gas_sc + produced_no_cap.gas_sc;

    let with_cap_water_accounted = final_with_cap.water_sc + produced_with_cap.water_sc;
    let with_cap_oil_accounted = final_with_cap.oil_sc + produced_with_cap.oil_sc;
    let with_cap_gas_accounted = final_with_cap.gas_sc + produced_with_cap.gas_sc;

    assert!(
        (no_cap_water_accounted - initial_no_cap.water_sc).abs() <= initial_no_cap.water_sc.max(1.0) * 5e-6,
        "zero-capillary water balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_no_cap.water_sc,
        no_cap_water_accounted
    );
    assert!(
        (no_cap_oil_accounted - initial_no_cap.oil_sc).abs() <= initial_no_cap.oil_sc.max(1.0) * 5e-3,
        "zero-capillary oil balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_no_cap.oil_sc,
        no_cap_oil_accounted
    );
    assert!(
        (no_cap_gas_accounted - initial_no_cap.gas_sc).abs() <= initial_no_cap.gas_sc.max(1.0) * 1e-2,
        "zero-capillary gas balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_no_cap.gas_sc,
        no_cap_gas_accounted
    );

    assert!(
        (with_cap_water_accounted - initial_with_cap.water_sc).abs() <= initial_with_cap.water_sc.max(1.0) * 5e-6,
        "capillary-entry water balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_with_cap.water_sc,
        with_cap_water_accounted
    );
    assert!(
        (with_cap_oil_accounted - initial_with_cap.oil_sc).abs() <= initial_with_cap.oil_sc.max(1.0) * 5e-3,
        "capillary-entry oil balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_with_cap.oil_sc,
        with_cap_oil_accounted
    );
    assert!(
        (with_cap_gas_accounted - initial_with_cap.gas_sc).abs() <= initial_with_cap.gas_sc.max(1.0) * 1e-2,
        "capillary-entry gas balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_with_cap.gas_sc,
        with_cap_gas_accounted
    );
}

#[test]
fn physics_gas_cap_free_gas_runtime_gravity_on_vs_off_changes_delivery() {
    let mut gravity_on = make_free_gas_cap_runtime_sim(0.0);
    let mut gravity_off = make_free_gas_cap_runtime_sim(0.0);
    gravity_off.set_gravity_enabled(false);

    let initial_on = total_component_inventory_sc_all_cells(&gravity_on);
    let initial_off = total_component_inventory_sc_all_cells(&gravity_off);

    for _ in 0..8 {
        gravity_on.step(0.5);
        gravity_off.step(0.5);

        assert!(
            gravity_on.last_solver_warning.is_empty(),
            "free-gas gas-cap gravity-on case emitted solver warning at t={}: {}",
            gravity_on.time_days,
            gravity_on.last_solver_warning
        );
        assert!(
            gravity_off.last_solver_warning.is_empty(),
            "free-gas gas-cap gravity-off case emitted solver warning at t={}: {}",
            gravity_off.time_days,
            gravity_off.last_solver_warning
        );
    }

    let top_id = gravity_on.idx(0, 0, 0);
    let bottom_id = gravity_on.idx(0, 0, 2);
    let produced_on = cumulative_component_production_sc(&gravity_on);
    let produced_off = cumulative_component_production_sc(&gravity_off);
    let final_on = total_component_inventory_sc_all_cells(&gravity_on);
    let final_off = total_component_inventory_sc_all_cells(&gravity_off);

    assert!(
        (gravity_on.sat_gas[bottom_id] - gravity_off.sat_gas[bottom_id]).abs() > 1e-2,
        "gravity-on gas-cap runtime should measurably change producer-layer gas arrival: on={:.6}, off={:.6}",
        gravity_on.sat_gas[bottom_id],
        gravity_off.sat_gas[bottom_id]
    );
    assert!(
        (produced_on.gas_sc - produced_off.gas_sc).abs() > 1e-1,
        "gravity-on gas-cap runtime should measurably change gas delivery to the producer over the short horizon: on={:.6}, off={:.6}",
        produced_on.gas_sc,
        produced_off.gas_sc
    );
    assert!(
        gravity_on.sat_gas[top_id].is_finite() && gravity_off.sat_gas[top_id].is_finite(),
        "cap-layer gas saturation should remain finite under gravity-on/off runtime comparison"
    );

    let on_water_accounted = final_on.water_sc + produced_on.water_sc;
    let on_oil_accounted = final_on.oil_sc + produced_on.oil_sc;
    let on_gas_accounted = final_on.gas_sc + produced_on.gas_sc;
    let off_water_accounted = final_off.water_sc + produced_off.water_sc;
    let off_oil_accounted = final_off.oil_sc + produced_off.oil_sc;
    let off_gas_accounted = final_off.gas_sc + produced_off.gas_sc;

    assert!(
        (on_water_accounted - initial_on.water_sc).abs() <= initial_on.water_sc.max(1.0) * 5e-6,
        "gravity-on water balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_on.water_sc,
        on_water_accounted
    );
    assert!(
        (on_oil_accounted - initial_on.oil_sc).abs() <= initial_on.oil_sc.max(1.0) * 5e-3,
        "gravity-on oil balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_on.oil_sc,
        on_oil_accounted
    );
    assert!(
        (on_gas_accounted - initial_on.gas_sc).abs() <= initial_on.gas_sc.max(1.0) * 1e-2,
        "gravity-on gas balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_on.gas_sc,
        on_gas_accounted
    );

    assert!(
        (off_water_accounted - initial_off.water_sc).abs() <= initial_off.water_sc.max(1.0) * 5e-6,
        "gravity-off water balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_off.water_sc,
        off_water_accounted
    );
    assert!(
        (off_oil_accounted - initial_off.oil_sc).abs() <= initial_off.oil_sc.max(1.0) * 5e-3,
        "gravity-off oil balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_off.oil_sc,
        off_oil_accounted
    );
    assert!(
        (off_gas_accounted - initial_off.gas_sc).abs() <= initial_off.gas_sc.max(1.0) * 1e-2,
        "gravity-off gas balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_off.gas_sc,
        off_gas_accounted
    );
}

#[test]
#[ignore = "explicit refinement probe: gravity-column evolution should stay stable under coarse-vs-fine timesteps"]
fn physics_gas_cap_gravity_column_timestep_refinement_keeps_profile_stable() {
    let case = GravityColumnCase {
        name: "refinement moderate capillary",
        perm_md: 20_000.0,
        initial_sw: 0.85,
        pc_entry_bar: 2.0,
    };
    let mut coarse = run_gravity_column_case(case, true);
    let mut fine = run_gravity_column_case(case, true);

    coarse.step(2.0);
    assert!(
        coarse.last_solver_warning.is_empty(),
        "coarse gas-cap refinement case emitted solver warning at t={}: {}",
        coarse.time_days,
        coarse.last_solver_warning
    );
    for _ in 0..4 {
        fine.step(0.5);
        assert!(
            fine.last_solver_warning.is_empty(),
            "fine gas-cap refinement case emitted solver warning at t={}: {}",
            fine.time_days,
            fine.last_solver_warning
        );
    }

    let coarse_top_pressure = coarse.pressure[coarse.idx(0, 0, 0)];
    let fine_top_pressure = fine.pressure[fine.idx(0, 0, 0)];
    let coarse_bottom_pressure = coarse.pressure[coarse.idx(0, 0, 2)];
    let fine_bottom_pressure = fine.pressure[fine.idx(0, 0, 2)];
    let coarse_top_sw = coarse.sat_water[coarse.idx(0, 0, 0)];
    let fine_top_sw = fine.sat_water[fine.idx(0, 0, 0)];

    let top_pressure_rel_diff =
        ((coarse_top_pressure - fine_top_pressure) / fine_top_pressure.max(1e-12)).abs();
    let bottom_pressure_rel_diff =
        ((coarse_bottom_pressure - fine_bottom_pressure) / fine_bottom_pressure.max(1e-12)).abs();
    let top_sw_abs_diff = (coarse_top_sw - fine_top_sw).abs();

    assert!(
        top_pressure_rel_diff <= 0.01,
        "gas-cap gravity-column top-pressure drift too large under timestep refinement: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_top_pressure,
        fine_top_pressure,
        top_pressure_rel_diff
    );
    assert!(
        bottom_pressure_rel_diff <= 0.01,
        "gas-cap gravity-column bottom-pressure drift too large under timestep refinement: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_bottom_pressure,
        fine_bottom_pressure,
        bottom_pressure_rel_diff
    );
    assert!(
        top_sw_abs_diff <= 0.01,
        "gas-cap gravity-column top-cell Sw drift too large under timestep refinement: coarse={:.6}, fine={:.6}, abs_diff={:.6}",
        coarse_top_sw,
        fine_top_sw,
        top_sw_abs_diff
    );
}