//! Native-only FIM debugging tests.
//!
//! All tests are `#[ignore]` — run explicitly via `test-native.sh`.
//! They use `step_fim_verbose()` which prints per-substep and per-Newton-iteration
//! diagnostics to stderr, using the exact same equations as WASM.

use super::*;
use std::time::Instant;

// ─── Scenario builders ───────────────────────────────────────────────────────

/// Two-phase waterflood, pressure-controlled wells.
fn build_waterflood_pressure(nx: usize, ny: usize, nz: usize) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(nx, ny, nz, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions(10.0, 10.0, 1.0).unwrap();
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(300.0);
    sim.set_initial_saturation(0.1);
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
    sim.set_rock_properties(1e-6, 0.0, 1.0, 1.0).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(nz > 1);
    sim.set_permeability_per_layer(vec![2000.0; nz], vec![2000.0; nz], vec![200.0; nz])
        .unwrap();
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.set_target_well_rates(0.0, 0.0).unwrap();
    sim.set_well_bhp_limits(100.0, 500.0).unwrap();
    sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
    sim.add_well(nx - 1, ny - 1, 0, 100.0, 0.1, 0.0, false)
        .unwrap();
    sim
}

/// Frontend `sweep_areal` analog: 21×21×1 pressure-controlled five-spot.
fn build_areal_sweep_pressure() -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(21, 21, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions(20.0, 20.0, 10.0).unwrap();
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(300.0);
    sim.set_initial_saturation(0.1);
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
    sim.set_rock_properties(1e-6, 0.0, 1.0, 1.0).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(false);
    sim.set_permeability_per_layer(vec![200.0], vec![200.0], vec![20.0])
        .unwrap();
    sim.set_stability_params(0.01, 50.0, 1.0);
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.set_target_well_rates(0.0, 0.0).unwrap();
    sim.set_well_bhp_limits(100.0, 500.0).unwrap();
    sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
    sim.add_well(20, 20, 0, 100.0, 0.1, 0.0, false).unwrap();
    sim
}

/// Two-phase waterflood, rate-controlled wells.
fn build_waterflood_rate(nx: usize, ny: usize, nz: usize) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(nx, ny, nz, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions(10.0, 10.0, 1.0).unwrap();
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(300.0);
    sim.set_initial_saturation(0.1);
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
    sim.set_rock_properties(1e-6, 0.0, 1.0, 1.0).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(nz > 1);
    sim.set_permeability_per_layer(vec![2000.0; nz], vec![2000.0; nz], vec![200.0; nz])
        .unwrap();
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_rate_controlled_wells(true);
    sim.set_injected_fluid("water").unwrap();
    sim.set_well_control_modes("rate".to_string(), "rate".to_string());
    sim.set_target_well_rates(10.0, 10.0).unwrap();
    sim.set_well_bhp_limits(100.0, 500.0).unwrap();
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
    sim.add_well(nx - 1, ny - 1, 0, 200.0, 0.1, 0.0, false)
        .unwrap();
    sim
}

/// Three-phase SPE1-like depletion, single producer + gas injector (10×10×3).
fn build_spe1_depletion() -> ReservoirSimulator {
    let mut sim = make_spe1_like_base_sim();
    sim.set_fim_enabled(true);
    sim
}

/// Three-phase gas injection into an oil reservoir.
fn build_gas_injection(nx: usize, ny: usize, nz: usize) -> ReservoirSimulator {
    build_gas_injection_variant(nx, ny, nz, nz > 1, true, true)
}

fn build_gas_injection_variant(
    nx: usize,
    ny: usize,
    nz: usize,
    gravity_enabled: bool,
    capillary_enabled: bool,
    rate_controlled: bool,
) -> ReservoirSimulator {
    use crate::pvt::{PvtRow, PvtTable};

    let mut sim = ReservoirSimulator::new(nx, ny, nz, 0.25);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions(10.0, 10.0, 5.0).unwrap();
    sim.set_initial_pressure(200.0);
    sim.set_initial_saturation(0.15);
    sim.set_fluid_properties(0.8, 0.4).unwrap();
    sim.set_fluid_compressibilities(1e-4, 5e-5).unwrap();
    sim.set_rock_properties(4e-5, 2500.0, 1.2, 1.0).unwrap();
    sim.set_fluid_densities(850.0, 1020.0).unwrap();
    sim.set_gas_fluid_properties(0.02, 1e-4, 0.8).unwrap();
    if capillary_enabled {
        sim.set_capillary_params(0.0, 2.0).unwrap();
    } else {
        sim.set_capillary_params(0.0, 1e-6).unwrap();
    }
    sim.set_gravity_enabled(gravity_enabled);
    sim.set_permeability_per_layer(vec![500.0; nz], vec![500.0; nz], vec![50.0; nz])
        .unwrap();
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 50.0,
                rs_m3m3: 20.0,
                bo_m3m3: 1.1,
                mu_o_cp: 1.0,
                bg_m3m3: 0.02,
                mu_g_cp: 0.015,
            },
            PvtRow {
                p_bar: 150.0,
                rs_m3m3: 80.0,
                bo_m3m3: 1.25,
                mu_o_cp: 0.7,
                bg_m3m3: 0.008,
                mu_g_cp: 0.018,
            },
            PvtRow {
                p_bar: 250.0,
                rs_m3m3: 140.0,
                bo_m3m3: 1.4,
                mu_o_cp: 0.5,
                bg_m3m3: 0.005,
                mu_g_cp: 0.022,
            },
            PvtRow {
                p_bar: 350.0,
                rs_m3m3: 200.0,
                bo_m3m3: 1.55,
                mu_o_cp: 0.4,
                bg_m3m3: 0.004,
                mu_g_cp: 0.025,
            },
        ],
        sim.pvt.c_o,
    ));
    sim.set_initial_rs(80.0);
    sim.set_three_phase_rel_perm_props(0.15, 0.15, 0.05, 0.05, 0.2, 2.0, 2.0, 2.0, 1e-5, 1.0, 0.95)
        .unwrap();
    sim.set_three_phase_mode_enabled(true);
    let _ = sim.set_three_phase_rel_perm_props(0.1, 0.1, 0.05, 0.05, 0.15, 2.0, 2.0, 2.0, 1.0, 1.0, 0.8);
    sim.set_injected_fluid("gas").unwrap();
    sim.set_gas_redissolution_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_rate_controlled_wells(rate_controlled);
    if rate_controlled {
        sim.set_well_control_modes("rate".to_string(), "rate".to_string());
        sim.set_target_well_rates(500.0, 200.0).unwrap();
    } else {
        sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
        sim.set_target_well_rates(0.0, 0.0).unwrap();
    }
    sim.set_well_bhp_limits(50.0, 400.0).unwrap();
    sim.add_well(0, 0, 0, 350.0, 0.1, 0.0, true).unwrap();
    sim.add_well(nx - 1, ny - 1, 0, 100.0, 0.1, 0.0, false)
        .unwrap();
    sim
}

// ─── Runner ──────────────────────────────────────────────────────────────────

fn run_verbose(label: &str, sim: &mut ReservoirSimulator, dt_days: f64, n_steps: usize) {
    eprintln!(
        "═══ {} ═══  (nx={} ny={} nz={}, cells={}, dt={}, steps={})",
        label,
        sim.nx,
        sim.ny,
        sim.nz,
        sim.nx * sim.ny * sim.nz,
        dt_days,
        n_steps
    );
    let started = Instant::now();
    for step in 0..n_steps {
        eprintln!(
            "─── outer step {}/{} at t={:.4} days ───",
            step + 1,
            n_steps,
            sim.time_days
        );
        sim.step_fim_verbose(dt_days);
        if let Some(point) = sim.rate_history.last() {
            let water_rate = (point.total_production_liquid - point.total_production_oil).max(0.0);
            let watercut = if point.total_production_liquid > 1e-12 {
                (water_rate / point.total_production_liquid).clamp(0.0, 1.0)
            } else {
                0.0
            };
            eprintln!(
                "    summary: time={:.2}d oil={:.2} water={:.2} liq={:.2} wc={:.4} avg_p={:.2} bhp_lim(prod/inj)=({:.2}/{:.2})",
                point.time,
                point.total_production_oil,
                water_rate,
                point.total_production_liquid,
                watercut,
                point.avg_reservoir_pressure,
                point.producer_bhp_limited_fraction,
                point.injector_bhp_limited_fraction,
            );
        }
        if !sim.last_solver_warning.is_empty() {
            eprintln!("*** WARNING: {}", sim.last_solver_warning);
            break;
        }
    }
    let elapsed = started.elapsed();
    eprintln!(
        "═══ {} done: t={:.4} days, {} history entries, {:.1} ms ═══\n",
        label,
        sim.time_days,
        sim.rate_history.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

// ─── 1D waterflood (pressure-controlled) ────────────────────────────────────

#[test]
#[ignore = "native FIM debug: waterflood pressure 24×1×1"]
fn fim_debug_wf_p_24() {
    let mut sim = build_waterflood_pressure(24, 1, 1);
    run_verbose("wf_p_24x1x1", &mut sim, 0.25, 20);
}

#[test]
#[ignore = "native FIM debug: waterflood pressure 48×1×1"]
fn fim_debug_wf_p_48() {
    let mut sim = build_waterflood_pressure(48, 1, 1);
    run_verbose("wf_p_48x1x1", &mut sim, 0.25, 20);
}

#[test]
#[ignore = "native FIM debug: waterflood pressure 100×1×1"]
fn fim_debug_wf_p_100() {
    let mut sim = build_waterflood_pressure(100, 1, 1);
    run_verbose("wf_p_100x1x1", &mut sim, 0.25, 20);
}

// ─── 2D waterflood (pressure-controlled) ────────────────────────────────────

#[test]
#[ignore = "native FIM debug: waterflood pressure 12×12×1"]
fn fim_debug_wf_p_12x12() {
    let mut sim = build_waterflood_pressure(12, 12, 1);
    run_verbose("wf_p_12x12x1", &mut sim, 0.5, 20);
}

#[test]
#[ignore = "native FIM debug: waterflood pressure 24×24×1"]
fn fim_debug_wf_p_24x24() {
    let mut sim = build_waterflood_pressure(24, 24, 1);
    run_verbose("wf_p_24x24x1", &mut sim, 0.5, 20);
}

// ─── 3D waterflood (pressure-controlled) ────────────────────────────────────

#[test]
#[ignore = "native FIM debug: waterflood pressure 12×12×3"]
fn fim_debug_wf_p_12x12x3() {
    let mut sim = build_waterflood_pressure(12, 12, 3);
    run_verbose("wf_p_12x12x3", &mut sim, 0.5, 20);
}

#[test]
#[ignore = "native FIM debug: waterflood pressure 24×24×2"]
fn fim_debug_wf_p_24x24x2() {
    let mut sim = build_waterflood_pressure(24, 24, 2);
    run_verbose("wf_p_24x24x2", &mut sim, 0.5, 20);
}

// ─── Waterflood rate-controlled ─────────────────────────────────────────────

#[test]
#[ignore = "native FIM debug: waterflood rate 24×1×1"]
fn fim_debug_wf_r_24() {
    let mut sim = build_waterflood_rate(24, 1, 1);
    run_verbose("wf_r_24x1x1", &mut sim, 0.25, 20);
}

#[test]
#[ignore = "native FIM debug: waterflood rate 12×12×1"]
fn fim_debug_wf_r_12x12() {
    let mut sim = build_waterflood_rate(12, 12, 1);
    run_verbose("wf_r_12x12x1", &mut sim, 0.5, 20);
}

#[test]
#[ignore = "native FIM debug: waterflood rate 12×12×3"]
fn fim_debug_wf_r_12x12x3() {
    let mut sim = build_waterflood_rate(12, 12, 3);
    run_verbose("wf_r_12x12x3", &mut sim, 0.5, 20);
}

// ─── Waterflood breakthrough ────────────────────────────────────────────────

#[test]
#[ignore = "native FIM debug: waterflood breakthrough 24×1×1"]
fn fim_debug_wf_bt_24() {
    let mut sim = build_waterflood_pressure(24, 1, 1);
    run_verbose("wf_bt_24x1x1", &mut sim, 0.5, 40);
}

#[test]
#[ignore = "native FIM debug: waterflood breakthrough 48×1×1"]
fn fim_debug_wf_bt_48() {
    let mut sim = build_waterflood_pressure(48, 1, 1);
    run_verbose("wf_bt_48x1x1", &mut sim, 50.0, 40);
}

#[test]
#[ignore = "native FIM debug: waterflood breakthrough 12×12×1"]
fn fim_debug_wf_bt_12x12() {
    let mut sim = build_waterflood_pressure(12, 12, 1);
    run_verbose("wf_bt_12x12x1", &mut sim, 1.0, 30);
}

#[test]
#[ignore = "native FIM debug: waterflood breakthrough 12×12×3"]
fn fim_debug_wf_bt_12x12x3() {
    let mut sim = build_waterflood_pressure(12, 12, 3);
    run_verbose("wf_bt_12x12x3", &mut sim, 1.0, 30);
}

#[test]
#[ignore = "native FIM debug: frontend sweep_areal baseline 21×21×1"]
fn fim_debug_sweep_areal() {
    let mut sim = build_areal_sweep_pressure();
    run_verbose("sweep_areal_21x21x1", &mut sim, 5.0, 50);
}

// ─── SPE1 depletion (three-phase, 10×10×3) ─────────────────────────────────

#[test]
#[ignore = "native FIM debug: SPE1 depletion 10×10×3"]
fn fim_debug_spe1_depletion() {
    let mut sim = build_spe1_depletion();
    run_verbose("spe1_depletion_10x10x3", &mut sim, 10.0, 10);
}

// ─── Gas injection ──────────────────────────────────────────────────────────

#[test]
#[ignore = "native FIM debug: gas injection 24×1×1"]
fn fim_debug_gas_24() {
    let mut sim = build_gas_injection(24, 1, 1);
    run_verbose("gas_24x1x1", &mut sim, 1.0, 20);
}

#[test]
#[ignore = "native FIM debug: gas injection 12×12×1"]
fn fim_debug_gas_12x12() {
    let mut sim = build_gas_injection(12, 12, 1);
    run_verbose("gas_12x12x1", &mut sim, 1.0, 20);
}

#[test]
#[ignore = "native FIM debug: gas injection 12×12×3"]
fn fim_debug_gas_12x12x3() {
    let mut sim = build_gas_injection(12, 12, 3);
    run_verbose("gas_12x12x3", &mut sim, 1.0, 20);
}

#[test]
#[ignore = "native FIM debug: gas injection 10×10×3"]
fn fim_debug_gas_10x10x3() {
    let mut sim = build_gas_injection(10, 10, 3);
    run_verbose("gas_10x10x3", &mut sim, 2.0, 15);
}

#[test]
#[ignore = "native FIM debug: gas injection 10×10×3 without gravity"]
fn fim_debug_gas_10x10x3_no_gravity() {
    let mut sim = build_gas_injection_variant(10, 10, 3, false, true, true);
    run_verbose("gas_10x10x3_no_gravity", &mut sim, 2.0, 15);
}

#[test]
#[ignore = "native FIM debug: gas injection 10×10×3 without capillary"]
fn fim_debug_gas_10x10x3_no_capillary() {
    let mut sim = build_gas_injection_variant(10, 10, 3, true, false, true);
    run_verbose("gas_10x10x3_no_capillary", &mut sim, 2.0, 15);
}

#[test]
#[ignore = "native FIM debug: gas injection 10×10×3 pressure-controlled wells"]
fn fim_debug_gas_10x10x3_pressure() {
    let mut sim = build_gas_injection_variant(10, 10, 3, true, true, false);
    run_verbose("gas_10x10x3_pressure", &mut sim, 2.0, 15);
}
#[test]
fn test_gas_injection_rate_print() {
    // just dummy
}
