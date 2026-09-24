//! Wellbore hydrostatic datum on multi-layer completions.
//!
//! Every completion of a physical well carries the same `bhp`, which is quoted
//! at one datum depth. The pressure a completion's connection law works against
//! is that datum value carried down the wellbore, `bhp + ρ_wb·g·(z_k − z_datum)`.
//!
//! The oracle here is exact rather than empirical: put a single-phase column in
//! hydrostatic equilibrium and stand a wellbore of the *same* fluid in it. Both
//! columns then have the same gradient, so a fully perforated well must see the
//! identical drawdown at every completion — any spread in the per-completion
//! rates is the modelling artefact this datum term exists to remove.

use crate::ReservoirSimulator;

const GRAVITY_M_S2: f64 = 9.806_65;
const LAYERS: usize = 4;
const LAYER_THICKNESS_M: f64 = 10.0;
const DATUM_PRESSURE_BAR: f64 = 300.0;
const PRODUCER_BHP_BAR: f64 = 290.0;

/// 1x1x4 water-filled column, gravity on, standing in exact hydrostatic
/// equilibrium, with one fully perforated producer.
fn hydrostatic_water_column() -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(1, 1, LAYERS, 0.2);
    sim.set_cell_dimensions_per_layer(20.0, 20.0, vec![LAYER_THICKNESS_M; LAYERS])
        .unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_initial_pressure(DATUM_PRESSURE_BAR);
    // Single mobile phase, so the produced mixture in the wellbore is water and
    // nothing else: the wellbore column and the reservoir column are the same
    // fluid by construction.
    sim.set_initial_saturation(1.0);
    sim.pc.p_entry = 0.0;
    sim.set_gravity_enabled(true);

    // Impose the equilibrium profile directly rather than relaxing into it, so
    // the reservoir gradient is the analytic ρ·g·Δz to machine precision.
    for k in 0..LAYERS {
        let id = sim.idx(0, 0, k);
        let depth_offset_m = sim.depth_at_k(k) - sim.depth_at_k(0);
        sim.pressure[id] = DATUM_PRESSURE_BAR
            + sim.water_density_generic(DATUM_PRESSURE_BAR) * GRAVITY_M_S2 * depth_offset_m * 1e-5;
    }

    for k in 0..LAYERS {
        sim.add_well_with_id(
            0,
            0,
            k,
            PRODUCER_BHP_BAR,
            0.1,
            0.0,
            false,
            "prod".to_string(),
        )
        .unwrap();
    }
    sim.refresh_well_head_offsets();
    sim
}

/// Reservoir-condition rate of each completion at the well's configured BHP.
fn completion_rates(sim: &ReservoirSimulator) -> Vec<f64> {
    (0..sim.wells.len())
        .map(|idx| {
            let well = &sim.wells[idx];
            let cell = sim.idx(well.i, well.j, well.k);
            sim.completion_rate_for_bhp(well, sim.pressure[cell], well.bhp)
                .expect("finite completion rate")
        })
        .collect()
}

fn spread(rates: &[f64]) -> f64 {
    let max = rates.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min = rates.iter().cloned().fold(f64::INFINITY, f64::min);
    max / min
}

#[test]
fn physics_wellbore_datum_equalizes_drawdown_down_a_hydrostatic_column() {
    let sim = hydrostatic_water_column();

    // Every completion has the same permeability, thickness and radius, so equal
    // drawdown must mean equal rate.
    let rates = completion_rates(&sim);
    assert!(
        rates.iter().all(|rate| *rate > 0.0),
        "producer should flow from every completion: {:?}",
        rates
    );
    assert!(
        spread(&rates) < 1.001,
        "datum-corrected completion rates should be uniform down a hydrostatic column, got {:?}",
        rates
    );

    // Same model with the head suppressed — the pre-fix behaviour — must show
    // the artefact the correction removes. Over 30 m of water that is
    // ρ·g·H = 2.94 bar of spurious extra drawdown on a 10 bar drawdown.
    let mut without_datum = hydrostatic_water_column();
    for well in without_datum.wells.iter_mut() {
        well.head_offset_bar = 0.0;
    }
    let uncorrected = completion_rates(&without_datum);
    assert!(
        spread(&uncorrected) > 1.25,
        "suppressing the head should reintroduce the depth-allocation artefact, got {:?}",
        uncorrected
    );
}

#[test]
fn physics_wellbore_datum_head_is_zero_without_gravity() {
    let mut sim = hydrostatic_water_column();
    sim.set_gravity_enabled(false);

    for well in sim.wells.iter() {
        assert_eq!(
            well.head_offset_bar, 0.0,
            "gravity-free models must be bit-identical to the pre-datum engine"
        );
    }
}

#[test]
fn physics_wellbore_datum_defaults_to_the_shallowest_completion() {
    let sim = hydrostatic_water_column();

    assert_eq!(
        sim.wells[0].head_offset_bar, 0.0,
        "the shallowest completion sits at the default datum, so carries no head"
    );

    // The derived column density is the average over the completions, each at
    // its own pressure, so it sits a few ppm off the datum-pressure value by
    // water compressibility alone — hence the 1e-4 bar tolerance rather than an
    // exact comparison.
    let expected_gradient_bar_per_m =
        sim.water_density_generic(DATUM_PRESSURE_BAR) * GRAVITY_M_S2 * 1e-5;
    for k in 1..LAYERS {
        let expected = expected_gradient_bar_per_m * (k as f64) * LAYER_THICKNESS_M;
        assert!(
            (sim.wells[k].head_offset_bar - expected).abs() < 1e-4,
            "completion {} head should be ρ·g·Δz = {} bar, got {}",
            k,
            expected,
            sim.wells[k].head_offset_bar
        );
    }
}

#[test]
fn physics_wellbore_datum_honours_explicit_datum_and_density() {
    let mut sim = hydrostatic_water_column();
    // Quote the BHP at the *bottom* completion instead, with a light wellbore
    // column that no longer matches the reservoir fluid.
    let bottom_depth_m = sim.depth_at_k(LAYERS - 1);
    for well in sim.wells.iter_mut() {
        well.datum_depth_m = Some(bottom_depth_m);
        well.wellbore_density_kg_m3 = Some(500.0);
    }
    sim.refresh_well_head_offsets();

    assert_eq!(
        sim.wells[LAYERS - 1].head_offset_bar,
        0.0,
        "the completion at the datum carries no head"
    );
    for k in 0..LAYERS {
        let expected = 500.0 * GRAVITY_M_S2 * (sim.depth_at_k(k) - bottom_depth_m) * 1e-5;
        assert!(
            (sim.wells[k].head_offset_bar - expected).abs() < 1e-9,
            "completion {} should use the explicit datum and density: expected {}, got {}",
            k,
            expected,
            sim.wells[k].head_offset_bar
        );
    }
    assert!(
        sim.wells[0].head_offset_bar < 0.0,
        "completions above the datum must carry a negative head"
    );
}

/// The shipped `wf_gravity` section (30x1x20, 5 D isotropic, 300 bar start, producer on a 200 bar
/// BHP, injector on a 40 m³/day rate as in its `ng_gravity` variant) with both wells perforated
/// in all twenty layers.
fn fully_perforated_gravity_section(vertical_perm_md: f64, gravity: bool) -> ReservoirSimulator {
    let (nx, nz) = (30usize, 20usize);
    let mut sim = ReservoirSimulator::new(nx, 1, nz, 0.2);
    sim.set_fim_enabled(false);
    sim.set_cell_dimensions(10.0, 20.0, 2.0).unwrap();
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(300.0);
    sim.set_initial_saturation(0.1);
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
    sim.set_rock_properties(1e-6, 0.0, 1.0, 1.0).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(gravity);
    sim.set_permeability_per_layer(
        vec![5000.0; nz],
        vec![5000.0; nz],
        vec![vertical_perm_md; nz],
    )
    .unwrap();
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_well_control_modes("rate".to_string(), "pressure".to_string());
    sim.set_target_well_rates(40.0, 0.0).unwrap();
    sim.set_well_bhp_limits(200.0, 600.0).unwrap();
    for k in 0..nz {
        sim.add_well_with_id(0, 0, k, 600.0, 0.1, 0.0, true, "injector".to_string())
            .unwrap();
        sim.add_well_with_id(nx - 1, 0, k, 200.0, 0.1, 0.0, false, "producer".to_string())
            .unwrap();
    }
    sim
}

/// #10: a fully perforated, rate-controlled well must not stall IMPES.
///
/// The injector spreads its fixed 40 m³/day over twenty completions, and that split moves with
/// every pressure change: a completion that took nothing at the old pressure takes several
/// m³/day at the new one. The IMPES well-rate limiter used to measure each completion alone,
/// against a 1 m³/day floor, so it read a 750% change and cut dt tenfold at any dt while the
/// well's total never moved. The first step exhausted the 32-retry budget at t ≈ 3e-5 d with
/// gravity on and isotropic k, and at t ≈ 0.015 d with k_v = 50 mD. Gravity is only one way to
/// shift the split: the gravity-off, tight-k_v case failed too, at t ≈ 4.8 d on the shipped
/// schedule. The limiter now measures each physical well's summed rate.
#[test]
fn physics_wellbore_fully_perforated_rate_controlled_well_does_not_stall_impes() {
    for (vertical_perm_md, gravity) in [(5000.0, true), (50.0, true), (50.0, false)] {
        let mut sim = fully_perforated_gravity_section(vertical_perm_md, gravity);
        for _ in 0..2 {
            sim.step(8.0);
            assert!(
                sim.last_solver_warning.is_empty(),
                "k_v={vertical_perm_md} gravity={gravity}: {} (after {} substeps)",
                sim.last_solver_warning,
                sim.rate_history.len()
            );
        }
        assert!((sim.time_days - 16.0).abs() < 1e-9);
    }
}
