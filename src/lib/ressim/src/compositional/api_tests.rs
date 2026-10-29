//! C13's engine-boundary contract (`comp_api_*`).
//!
//! These are the tests the plan asks for when it says to expose restart "only through a tested
//! API". They run the same code the WASM shell runs, natively, so a WASM/native difference can
//! only come from the shell — which has nothing in it.

use super::api::*;
use super::assembly::Face;
use crate::fluid::flash::flash;
use crate::fluid::pinned;
use crate::fluid::units::bar_to_pa;

/// The 1D displacement C12 validated, as a payload. Five cells, the pinned ternary, a rate-limited
/// injector against a BHP producer.
fn valid_config() -> CaseConfig {
    CaseConfig {
        schema: CASE_SCHEMA.to_string(),
        fluid: FluidChoice::PinnedTernary,
        grid: GridConfig {
            cells: 5,
            dx_m: 60.0,
            dy_m: 6.0,
            dz_m: 6.0,
            porosity: 0.1,
            permeability_md: 100.0,
            rock_reference_pressure_bar: 68.9476,
            rock_compressibility_per_bar: 0.0,
            box_grid: None,
        },
        relperm: RelPermConfig::Linear,
        initial_pressure_bar: 75.0,
        initial_composition: vec![0.1, 0.3, 0.6],
        wells: vec![
            WellConfig {
                id: "INJ".to_string(),
                completions: vec![CompletionConfig {
                    cell: 0,
                    well_index: 4.58,
                    head_offset_bar: 0.0,
                }],
                control: WellControlConfig::Bhp { target_bar: 150.0 },
                injection_composition: Some(vec![1.0, 0.0, 0.0]),
                inject_water: false,
            },
            WellConfig {
                id: "PROD".to_string(),
                completions: vec![CompletionConfig {
                    cell: 4,
                    well_index: 4.58,
                    head_offset_bar: 0.0,
                }],
                control: WellControlConfig::Bhp { target_bar: 50.0 },
                injection_composition: None,
                inject_water: false,
            },
        ],
        gravity_enabled: false,
        initial_pressure_gradient_bar_per_m: None,
        water: None,
    }
}

#[test]
fn comp_api_a_valid_config_builds_and_steps() {
    let mut case = CompositionalCase::new(valid_config()).expect("the C12 case must build");
    assert_eq!(case.time_days(), 0.0);

    let outcome = case.step(0.05);
    assert!(
        outcome.succeeded(),
        "the first step failed: {:?}",
        outcome.failure
    );
    assert_eq!(outcome.accepted_dt_days, Some(0.05));
    assert!((case.time_days() - 0.05).abs() < 1e-12);
    assert!(
        outcome.newton_iterations > 0,
        "a step that solved nothing is not a step"
    );
}

/// The uniform column is the geometry every validated case before #47 used, and adding a box grid
/// must not move it: same faces, same order, same transmissibility, gravity off.
#[test]
fn comp_api_the_uniform_column_geometry_is_unchanged() {
    let case = CompositionalCase::new(valid_config()).unwrap();
    assert_eq!(case.dimensions(), [5, 1, 1]);
    assert_eq!(case.faces().len(), 4);
    let expected = 8.526_988_8e-3 * 100.0 * (6.0 * 6.0) / 60.0;
    for (index, face) in case.faces().iter().enumerate() {
        assert_eq!(face.cell_i, index);
        assert_eq!(face.cell_j, index + 1);
        assert_eq!(face.geom_t, expected);
        assert_eq!(face.gravity, super::flux::Gravity::OFF);
    }
    for volume in case.pore_volumes() {
        assert_eq!(*volume, 60.0 * 6.0 * 6.0 * 0.1);
    }
}

fn box_config() -> CaseConfig {
    let mut config = valid_config();
    config.grid.cells = 8;
    config.grid.dx_m = 30.0;
    config.grid.dy_m = 20.0;
    config.grid.dz_m = 10.0;
    config.grid.box_grid = Some(BoxGridConfig {
        nx: 2,
        ny: 2,
        nz: 2,
        layer_dz_m: vec![10.0, 20.0],
        layer_kx_md: vec![100.0, 200.0],
        layer_ky_md: vec![110.0, 210.0],
        layer_kz_md: vec![10.0, 20.0],
        top_depth_m: 1000.0,
    });
    config.gravity_enabled = true;
    config.initial_pressure_gradient_bar_per_m = Some(0.1);
    config.wells[0].completions[0].cell = 0;
    config.wells[1].completions[0].cell = 7;
    config
}

/// The box has TPFA faces in all three directions, per-layer thickness and permeability, and a
/// depth on every vertical face. Index order is `(k, j, i)`, then x, y, z.
#[test]
fn comp_api_a_box_grid_carries_three_directions_and_layer_properties() {
    let case = CompositionalCase::new(box_config()).unwrap();
    assert_eq!(case.dimensions(), [2, 2, 2]);
    assert_eq!(case.pore_volumes().len(), 8);
    for (cell, volume) in case.pore_volumes().iter().enumerate() {
        let layer = cell / 4;
        assert_eq!(*volume, 30.0 * 20.0 * [10.0, 20.0][layer] * 0.1);
    }

    // 1 x-face, 1 y-face and 1 z-face behind every cell except the last layer/dimension.
    assert_eq!(case.faces().len(), 12);
    let d = 8.526_988_8e-3;
    assert_eq!(
        case.faces()[0],
        Face {
            cell_i: 0,
            cell_j: 1,
            geom_t: d * 100.0 * (20.0 * 10.0) / 30.0,
            gravity: super::flux::Gravity::OFF,
        }
    );
    assert_eq!(
        case.faces()[1],
        Face {
            cell_i: 0,
            cell_j: 2,
            geom_t: d * 110.0 * (30.0 * 10.0) / 20.0,
            gravity: super::flux::Gravity::OFF,
        }
    );
    assert_eq!(
        case.faces()[2],
        Face {
            cell_i: 0,
            cell_j: 4,
            geom_t: d * 2.0 * 10.0 * 20.0 * 30.0 * 20.0 / (20.0 * 10.0 + 10.0 * 20.0),
            gravity: super::flux::Gravity::between(1005.0, 1020.0),
        }
    );
    // Layer 1 starts at face 8, after layer 0's 4 x/y faces and 4 z faces.
    assert_eq!(case.faces()[8].cell_i, 4);
    assert_eq!(case.faces()[8].cell_j, 5);
    assert_eq!(case.faces()[8].geom_t, d * 200.0 * (20.0 * 20.0) / 30.0);
}

/// The assembled system runs in three dimensions with wells at opposite corners.
#[test]
fn comp_api_a_box_grid_steps_with_wells_in_three_directions() {
    let mut case = CompositionalCase::new(box_config()).unwrap();
    let outcome = case.step(0.01);
    assert!(
        outcome.succeeded(),
        "the first box-grid step failed: {:?}",
        outcome.failure
    );
    let snapshot = case.snapshot();
    assert_eq!(snapshot.pressure.len(), 8);
    assert_eq!(snapshot.composition[0].len(), 8);
}

/// A multi-completion producer carries each completion's own head offset through the payload. A
/// positive offset raises the connection pressure at a deeper completion, so the same shared BHP
/// draws less from the reservoir. This is the API-level counterpart of
/// `comp_well_multiperf_head_offsets_differentiate_the_connections`.
#[test]
fn comp_api_a_multi_completion_well_carries_its_head_offsets() {
    let produced = |deep_head_offset_bar: f64| {
        let mut config = valid_config();
        config.fluid = FluidChoice::PinnedBinary;
        config.grid.cells = 2;
        config.grid.dx_m = 10.0;
        config.grid.dy_m = 10.0;
        config.grid.dz_m = 10.0;
        config.grid.box_grid = Some(BoxGridConfig {
            nx: 1,
            ny: 1,
            nz: 2,
            layer_dz_m: vec![10.0, 10.0],
            layer_kx_md: vec![100.0, 100.0],
            layer_ky_md: vec![100.0, 100.0],
            layer_kz_md: vec![100.0, 100.0],
            top_depth_m: 2000.0,
        });
        config.relperm = RelPermConfig::Corey {
            liquid_residual: 0.0,
            vapour_residual: 0.0,
            liquid_exponent: 1.0,
            vapour_exponent: 1.0,
            liquid_endpoint: 1.0,
            vapour_endpoint: 1.0,
        };
        config.initial_pressure_bar = 200.0;
        config.initial_composition = vec![0.6, 0.4];
        config.wells = vec![WellConfig {
            id: "PROD".to_string(),
            completions: vec![
                CompletionConfig {
                    cell: 0,
                    well_index: 1.0,
                    head_offset_bar: 0.0,
                },
                CompletionConfig {
                    cell: 1,
                    well_index: 1.0,
                    head_offset_bar: deep_head_offset_bar,
                },
            ],
            control: WellControlConfig::Bhp { target_bar: 150.0 },
            injection_composition: None,
            inject_water: false,
        }];

        let mut case = CompositionalCase::new(config).unwrap();
        let outcome = case.step(0.01);
        assert!(
            outcome.succeeded(),
            "the multi-completion step failed: {:?}",
            outcome.failure
        );
        // Producers report negative cumulative moles; the magnitude is what was produced.
        -case.snapshot().cumulative_well_moles[0].iter().sum::<f64>()
    };

    let no_head = produced(0.0);
    let with_head = produced(1.0);
    assert!(no_head > 0.0 && with_head > 0.0);
    assert!(
        with_head < 0.995 * no_head,
        "a 1 bar head offset on one of two completions should reduce production: \
         {with_head} against {no_head}"
    );
}

/// A mis-specified box is rejected at the boundary, with the field named.
#[test]
#[allow(clippy::type_complexity)]
fn comp_api_structured_grid_misuse_is_named() {
    let cases: Vec<(&str, Box<dyn Fn(&mut CaseConfig)>)> = vec![
        (
            "grid.box_grid",
            Box::new(|c: &mut CaseConfig| {
                let mut config = box_config();
                config.grid.cells = 7;
                c.grid = config.grid;
            }),
        ),
        (
            "grid.box_grid.layer_dz_m",
            Box::new(|c: &mut CaseConfig| {
                let mut config = box_config();
                config.grid.box_grid.as_mut().unwrap().layer_dz_m = vec![10.0];
                c.grid = config.grid;
            }),
        ),
        (
            "initial_pressure_gradient_bar_per_m",
            Box::new(|c: &mut CaseConfig| {
                c.initial_pressure_gradient_bar_per_m = Some(0.1);
            }),
        ),
        (
            "initial_pressure_gradient_bar_per_m",
            Box::new(|c: &mut CaseConfig| {
                let mut config = box_config();
                config.initial_pressure_bar = 10.0;
                config.initial_pressure_gradient_bar_per_m = Some(-10.0);
                c.grid = config.grid;
                c.initial_pressure_bar = config.initial_pressure_bar;
                c.initial_pressure_gradient_bar_per_m = config.initial_pressure_gradient_bar_per_m;
            }),
        ),
    ];
    for (field, mutate) in cases {
        let mut config = valid_config();
        mutate(&mut config);
        let error = CompositionalCase::new(config)
            .err()
            .unwrap_or_else(|| panic!("{field} was accepted"));
        let message = error.to_string();
        assert!(
            message.contains(field),
            "the error for {field} does not name it: {message}"
        );
    }
}

/// The gradient field is applied at each layer's *centre*, measured from the top of layer 0. It is
/// the caller-supplied hydrostatic-style initialization; the stationary test below checks that a
/// profile built this way carries no flux.
#[test]
fn comp_api_the_initial_pressure_gradient_is_applied_per_layer() {
    let mut config = valid_config();
    config.grid.cells = 3;
    config.grid.dx_m = 10.0;
    config.grid.dy_m = 10.0;
    config.grid.dz_m = 10.0;
    config.grid.box_grid = Some(BoxGridConfig {
        nx: 1,
        ny: 1,
        nz: 3,
        layer_dz_m: vec![10.0, 20.0, 30.0],
        layer_kx_md: vec![100.0; 3],
        layer_ky_md: vec![100.0; 3],
        layer_kz_md: vec![100.0; 3],
        top_depth_m: 1000.0,
    });
    config.initial_pressure_bar = 100.0;
    config.initial_pressure_gradient_bar_per_m = Some(0.2);
    config.wells.clear();

    let snapshot = CompositionalCase::new(config).unwrap().snapshot();
    // Layer centres: 1005, 1020 and 1045 m; the pressure is the top value plus 0.2 bar/m.
    for (cell, expected) in [101.0, 104.0, 109.0].into_iter().enumerate() {
        assert!(
            (snapshot.pressure[cell] - expected).abs() < 1e-12,
            "cell {cell}: {} bar against {expected}",
            snapshot.pressure[cell]
        );
    }
}

/// A vertical single-phase column at the analytic hydrostatic pressure carries no flux and does
/// not move. This is #47's zero-flow gravity gate through the public case boundary.
#[test]
fn comp_api_a_hydrostatic_box_column_is_stationary() {
    let spec = pinned::binary().unwrap();
    let z = [0.6, 0.4];
    let dz = 5.0;
    let mut pressures = vec![400.0];
    for _ in 1..4 {
        let above = *pressures.last().unwrap();
        let mut below = above;
        for _ in 0..40 {
            let rho = |p: f64| {
                flash(
                    &spec,
                    bar_to_pa(p),
                    spec.reservoir_temperature_k(),
                    &z,
                    None,
                )
                .unwrap()
                .liquid
                .as_ref()
                .unwrap()
                .mass_density
            };
            below = above + 0.5 * (rho(above) + rho(below)) * 9.80665 * dz * 1e-5;
        }
        pressures.push(below);
    }

    let mut config = valid_config();
    config.fluid = FluidChoice::PinnedBinary;
    config.grid.cells = 4;
    config.grid.dx_m = 10.0;
    config.grid.dy_m = 10.0;
    config.grid.dz_m = dz;
    config.grid.box_grid = Some(BoxGridConfig {
        nx: 1,
        ny: 1,
        nz: 4,
        layer_dz_m: vec![dz; 4],
        layer_kx_md: vec![100.0; 4],
        layer_ky_md: vec![100.0; 4],
        layer_kz_md: vec![100.0; 4],
        top_depth_m: 2000.0,
    });
    config.relperm = RelPermConfig::Corey {
        liquid_residual: 0.0,
        vapour_residual: 0.0,
        liquid_exponent: 1.0,
        vapour_exponent: 1.0,
        liquid_endpoint: 1.0,
        vapour_endpoint: 1.0,
    };
    config.initial_composition = z.to_vec();
    config.initial_pressure_bar = 400.0;
    config.wells.clear();
    config.gravity_enabled = true;

    let case = CompositionalCase::new(config).unwrap();
    let mut checkpoint = case.checkpoint();
    checkpoint.pressure = pressures.clone();
    let mut case = CompositionalCase::restore(&checkpoint).unwrap();
    let before = case.snapshot();

    let outcome = case.step(1.0);
    assert!(
        outcome.succeeded(),
        "the hydrostatic column failed to step: {:?}",
        outcome.failure
    );
    let after = case.snapshot();
    for (cell, (a, b)) in before.pressure.iter().zip(&after.pressure).enumerate() {
        assert!(
            (a - b).abs() < 1e-8,
            "cell {cell} moved off hydrostatic equilibrium: {a} -> {b}"
        );
    }
    for component in 0..2 {
        for (cell, (a, b)) in before.composition[component]
            .iter()
            .zip(&after.composition[component])
            .enumerate()
        {
            assert!(
                (a - b).abs() < 1e-12,
                "cell {cell} component {component} moved: {a} -> {b}"
            );
        }
    }
}

/// **An absent or foreign discriminator is refused, never guessed at.** The plan is explicit that
/// an absent one must not be read as compositional.
#[test]
fn comp_api_a_foreign_schema_is_refused() {
    for schema in ["", "ressim-compositional-case/2", "black-oil"] {
        let mut config = valid_config();
        config.schema = schema.to_string();
        match CompositionalCase::new(config) {
            Err(ConfigError::UnsupportedSchema { found, expected }) => {
                assert_eq!(found, schema);
                assert_eq!(expected, CASE_SCHEMA);
            }
            other => panic!(
                "schema {schema:?} was not refused: {other:?}",
                other = other.err()
            ),
        }
    }
}

/// Everything unsupported is refused **before** a run is allocated, and the error names the field.
#[test]
fn comp_api_unsupported_input_is_named_not_silently_accepted() {
    let cases: Vec<(&str, Box<dyn Fn(&mut CaseConfig)>)> = vec![
        (
            "grid.porosity",
            Box::new(|c: &mut CaseConfig| c.grid.porosity = 0.0),
        ),
        (
            "grid.dx_m",
            Box::new(|c: &mut CaseConfig| c.grid.dx_m = -1.0),
        ),
        (
            "grid.permeability_md",
            Box::new(|c: &mut CaseConfig| c.grid.permeability_md = f64::NAN),
        ),
        (
            "initial_pressure_bar",
            Box::new(|c: &mut CaseConfig| c.initial_pressure_bar = 0.0),
        ),
        (
            "initial_composition sum",
            Box::new(|c: &mut CaseConfig| c.initial_composition = vec![0.1, 0.3, 0.5]),
        ),
        (
            "wells[INJ].completions.cell",
            Box::new(|c: &mut CaseConfig| c.wells[0].completions[0].cell = 99),
        ),
        (
            "wells[INJ].completions.well_index",
            Box::new(|c: &mut CaseConfig| c.wells[0].completions[0].well_index = 0.0),
        ),
    ];
    for (field, mutate) in cases {
        let mut config = valid_config();
        mutate(&mut config);
        let error = CompositionalCase::new(config)
            .err()
            .unwrap_or_else(|| panic!("{field} was accepted"));
        let message = error.to_string();
        assert!(
            message.contains(field),
            "the error for {field} does not name it: {message}"
        );
    }
}

/// A composition of the wrong length is a component-count mismatch, reported as one.
#[test]
fn comp_api_a_component_count_mismatch_is_reported_as_one() {
    let mut config = valid_config();
    config.fluid = FluidChoice::PinnedBinary;
    match CompositionalCase::new(config) {
        Err(ConfigError::CompositionMismatch {
            field,
            expected,
            found,
        }) => {
            assert_eq!(field, "initial_composition");
            assert_eq!((expected, found), (2, 3));
        }
        other => panic!(
            "a 3-component mixture on a binary fluid was accepted: {other:?}",
            other = other.err()
        ),
    }
}

/// `Linear` is admissible and **reported as verification-only**, so a scenario-admission check can
/// refuse to ship on it. See `docs/COMPOSITIONAL_VALIDATION.md` §6.
#[test]
fn comp_api_linear_relperm_is_flagged_as_verification_only() {
    let mut config = valid_config();
    assert!(
        config.validate().unwrap().relperm_is_verification_only,
        "Linear must be flagged"
    );

    config.relperm = RelPermConfig::Corey {
        liquid_residual: 0.1,
        vapour_residual: 0.05,
        liquid_exponent: 2.0,
        vapour_exponent: 2.0,
        liquid_endpoint: 0.9,
        vapour_endpoint: 0.8,
    };
    assert!(
        !config.validate().unwrap().relperm_is_verification_only,
        "a sourced Corey curve is not verification-only"
    );
}

/// An invalid Corey curve is refused by the model itself, and the reason travels.
#[test]
fn comp_api_an_invalid_relperm_is_refused_with_its_own_reason() {
    let mut config = valid_config();
    config.relperm = RelPermConfig::Corey {
        liquid_residual: 0.8,
        vapour_residual: 0.8,
        liquid_exponent: 2.0,
        vapour_exponent: 2.0,
        liquid_endpoint: 0.9,
        vapour_endpoint: 0.8,
    };
    match CompositionalCase::new(config) {
        Err(ConfigError::Rejected { field, reason }) => {
            assert_eq!(field, "relperm");
            assert!(!reason.is_empty(), "the underlying reason was dropped");
        }
        other => panic!(
            "residuals summing above one were accepted: {other:?}",
            other = other.err()
        ),
    }
}

/// The snapshot carries what a UI needs and **no derivative arrays**, which the plan asks for by
/// name. Its size is checked, because that is the property that stops being true by accident.
#[test]
fn comp_api_snapshot_is_the_state_and_its_totals_only() {
    let mut case = CompositionalCase::new(valid_config()).unwrap();
    assert!(case.step(0.05).succeeded());
    let snapshot = case.snapshot();

    assert_eq!(snapshot.component_ids.len(), 3);
    assert_eq!(snapshot.pressure.len(), 5);
    assert_eq!(snapshot.composition.len(), 3);
    for column in &snapshot.composition {
        assert_eq!(column.len(), 5, "composition is component-major");
    }
    assert_eq!(snapshot.phase_state.len(), 5);
    assert_eq!(snapshot.vapour_saturation.len(), 5);
    assert_eq!(snapshot.inventory.len(), 3);

    // Every cell's composition sums to one, which is the invariant the engine never renormalizes
    // into existence.
    for cell in 0..5 {
        let total: f64 = (0..3).map(|i| snapshot.composition[i][cell]).sum();
        assert!(
            (total - 1.0).abs() < 1e-9,
            "cell {cell} composition sums to {total}"
        );
    }

    // The whole payload is small: a snapshot is O(cells x components), not O(cells x components^2).
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(
        json.len() < 4000,
        "the snapshot is {} bytes for a 5-cell case; a derivative array has crept in",
        json.len()
    );
}

/// A failing step is **reported**, not panicked. A scenario has to be able to show this.
#[test]
fn comp_api_a_failing_step_is_reported_not_panicked() {
    let mut config = valid_config();
    // A vastly over-long step into a five-cell grid against a 150 bar injector: the lifecycle cuts
    // and eventually gives up rather than accepting something inadmissible.
    config.wells[0].control = WellControlConfig::Bhp { target_bar: 900.0 };
    let mut case = CompositionalCase::new(config).unwrap();
    let outcome = case.step(500.0);
    if !outcome.succeeded() {
        let failure = outcome.failure.expect("a failed step must say why");
        assert!(
            !failure.kind.is_empty() && !failure.message.is_empty(),
            "a failure with nothing in it is not actionable"
        );
        // And nothing moved.
        assert_eq!(case.time_days(), 0.0);
    }
}

/// **Checkpoint roundtrip.** The state and the clock come back, and the restored case continues
/// identically — which is the property a restart has to have and the one a serialization bug
/// breaks silently.
#[test]
fn comp_api_checkpoint_roundtrips_and_continues_identically() {
    let mut original = CompositionalCase::new(valid_config()).unwrap();
    for _ in 0..3 {
        assert!(original.step(0.02).succeeded());
    }

    let json = serde_json::to_string(&original.checkpoint()).unwrap();
    let parsed: Checkpoint = serde_json::from_str(&json).unwrap();
    let mut restored = CompositionalCase::restore(&parsed).expect("the checkpoint must restore");

    assert_eq!(restored.time_days(), original.time_days());
    let a = original.snapshot();
    let b = restored.snapshot();
    assert_eq!(
        a.pressure, b.pressure,
        "pressures did not survive the roundtrip"
    );
    assert_eq!(a.composition, b.composition, "compositions did not survive");

    // And it continues the same way, which a state that merely *looks* restored would not.
    assert!(original.step(0.02).succeeded());
    assert!(restored.step(0.02).succeeded());
    let a = original.snapshot();
    let b = restored.snapshot();
    for cell in 0..5 {
        assert!(
            (a.pressure[cell] - b.pressure[cell]).abs() < 1e-9,
            "cell {cell} diverged after the restore: {} vs {}",
            a.pressure[cell],
            b.pressure[cell]
        );
    }
}

/// A checkpoint from another schema is refused rather than reinterpreted.
#[test]
fn comp_api_an_incompatible_checkpoint_is_refused() {
    let case = CompositionalCase::new(valid_config()).unwrap();
    let mut checkpoint = case.checkpoint();
    checkpoint.schema = "ressim-compositional-checkpoint/0".to_string();
    match CompositionalCase::restore(&checkpoint) {
        Err(ConfigError::UnsupportedSchema { found, expected }) => {
            assert_eq!(found, "ressim-compositional-checkpoint/0");
            assert_eq!(expected, CHECKPOINT_SCHEMA);
        }
        other => panic!(
            "an old checkpoint was accepted: {other:?}",
            other = other.err()
        ),
    }
}

/// The payload is JSON the worker boundary can carry: plain data, and it round-trips.
#[test]
fn comp_api_the_config_is_plain_serializable_data() {
    let config = valid_config();
    let json = serde_json::to_string(&config).unwrap();
    let parsed: CaseConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(
        parsed, config,
        "the config did not survive a JSON roundtrip"
    );
    // The discriminators are the readable ones a TypeScript union will use.
    assert!(
        json.contains("\"pinned-ternary\""),
        "fluid discriminator: {json}"
    );
    assert!(
        json.contains("\"model\":\"linear\""),
        "relperm discriminator: {json}"
    );
    assert!(
        json.contains("\"control\":\"bhp\""),
        "control discriminator: {json}"
    );
}
