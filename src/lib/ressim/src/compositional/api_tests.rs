//! C13's engine-boundary contract (`comp_api_*`).
//!
//! These are the tests the plan asks for when it says to expose restart "only through a tested
//! API". They run the same code the WASM shell runs, natively, so a WASM/native difference can
//! only come from the shell — which has nothing in it.

use super::api::*;

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
            },
        ],
        gravity_enabled: false,
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
