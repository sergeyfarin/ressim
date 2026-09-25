//! Initial-state inputs added for SPE1 fidelity (#57): a rock reference pressure separate from the
//! initial pressure (Eclipse `ROCK` item 1), and a hydrostatic oil-column initialization from a
//! datum (Eclipse `EQUIL` for an oil zone with no contact inside the grid).

use crate::ReservoirSimulator;

/// A 1x1x3 oil column, 10 m layers from 1000 m, gravity on, no wells, connate water.
fn oil_column(fim: bool) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(1, 1, 3, 0.2);
    sim.set_fim_enabled(fim);
    sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![10.0, 10.0, 10.0])
        .unwrap();
    sim.set_permeability_per_layer(vec![100.0; 3], vec![100.0; 3], vec![50.0; 3])
        .unwrap();
    sim.set_rel_perm_props(0.2, 0.2, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_saturation(0.2);
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_rock_properties(0.0, 1000.0, 1.0, 1.0).unwrap();
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(true);
    sim
}

/// The reference survives `set_initial_pressure` whichever is called first, and without the
/// call it still follows the initial pressure, as it always has.
#[test]
fn physics_initial_state_rock_reference_pressure_is_independent_of_initial_pressure() {
    let mut before = oil_column(true);
    before.set_rock_reference_pressure(1.01353).unwrap();
    before.set_initial_pressure(331.0);
    assert_eq!(before.rock_reference_pressure_bar, 1.01353);

    let mut after = oil_column(true);
    after.set_initial_pressure(331.0);
    after.set_rock_reference_pressure(1.01353).unwrap();
    after.set_initial_pressure(250.0);
    assert_eq!(after.rock_reference_pressure_bar, 1.01353);

    let mut default = oil_column(true);
    default.set_initial_pressure(331.0);
    assert_eq!(default.rock_reference_pressure_bar, 331.0);

    assert!(default.set_rock_reference_pressure(f64::NAN).is_err());
}

/// Dead oil at constant density (no PVT table, `c_o = 0`): the column is exactly linear,
/// `p = p_datum + rho·g·(z − z_datum)`, above and below the datum.
#[test]
fn physics_initial_state_hydrostatic_dead_oil_column_is_linear_in_depth() {
    let mut sim = oil_column(true);
    sim.set_fluid_compressibilities(0.0, 0.0).unwrap();
    sim.set_initial_pressure_hydrostatic(1012.0, 200.0).unwrap();
    for k in 0..3 {
        let depth = sim.depth_at_k(k);
        let expected = 200.0 + 800.0 * 9.80665 * (depth - 1012.0) * 1e-5;
        assert!(
            (sim.pressure[k] - expected).abs() < 1e-9,
            "layer {k} at {depth} m: {} vs {expected}",
            sim.pressure[k]
        );
    }
    // The PVT references are the datum's, as with `set_initial_pressure`.
    assert_eq!(sim.oil_pvt_reference_pressure_bar, 200.0);
    assert!(sim.set_initial_pressure_hydrostatic(1012.0, -1.0).is_err());
}

/// SPE1's live oil (its own setup, datum 8400 ft at 4800 psia): the initialized column is in
/// equilibrium, so with no wells nothing moves. A uniform initial pressure, as SPE1 used to start
/// from, is not: gravity sets up flow and the layers drift. Measured over 150 days: 1.1e-9 bar
/// from the hydrostatic start, 0.75 bar from the uniform one, on both solvers.
#[test]
fn physics_initial_state_hydrostatic_live_oil_column_stays_at_rest_on_both_solvers() {
    let column = |fim: bool| {
        let mut sim =
            crate::tests::make_spe1_like_grid_sim(1, 1, 0, 0, vec![200.0; 3], 0.05, 20.0, 0.2);
        sim.wells.clear();
        sim.set_fim_enabled(fim);
        sim
    };
    let drift = |mut sim: ReservoirSimulator| {
        let start = sim.pressure.clone();
        for _ in 0..5 {
            sim.step(30.0);
        }
        (0..3)
            .map(|k| (sim.pressure[k] - start[k]).abs())
            .fold(0.0, f64::max)
    };
    for fim in [true, false] {
        let mut hydrostatic = column(fim);
        hydrostatic
            .set_initial_pressure_hydrostatic(2560.32, 331.0)
            .unwrap();
        let p = &hydrostatic.pressure;
        assert!(
            p[0] < p[1] && p[1] < p[2],
            "pressure grows downwards: {p:?}"
        );
        let at_rest = drift(hydrostatic);
        let mut uniform_start = column(fim);
        uniform_start.set_initial_pressure(331.0);
        let uniform = drift(uniform_start);
        assert!(
            at_rest < 1e-3,
            "fim={fim}: hydrostatic column drifted {at_rest} bar"
        );
        assert!(
            uniform > 0.1,
            "fim={fim}: uniform column drifted only {uniform} bar"
        );
    }
}
