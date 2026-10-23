//! C1 contract tests for the fluid specification (`comp_spec_*`).
//!
//! Unit-conversion tests (`comp_units_*`) live next to the code they test in `units.rs`.

use super::fixture;
use super::pinned;
use super::specification::{
    Component, ComponentActivity, EosVariant, FluidSpecError, FluidSpecification, MAX_COMPONENTS,
    OverallComposition, SurfaceConditions, ViscosityModel,
};

/// The temperature the synthetic specifications below are built at. It is the C0 isotherm, but
/// nothing here depends on that: these tests are about validation, not thermodynamics.
const TEST_TEMPERATURE_K: f64 = 423.15;

fn comp(id: &str) -> Component {
    Component {
        id: id.to_string(),
        molar_mass_kg_per_mol: 0.016,
        critical_temperature_k: 190.6,
        critical_pressure_pa: 4.6e6,
        critical_volume_m3_per_kmol: 9.863e-2,
        acentric_factor: 0.011,
    }
}

fn spec_of(components: Vec<Component>) -> Result<FluidSpecification, FluidSpecError> {
    let n = components.len();
    FluidSpecification::new(
        components,
        vec![vec![0.0; n]; n],
        EosVariant::PengRobinson,
        ViscosityModel::LohrenzBrayClark,
        TEST_TEMPERATURE_K,
        None,
    )
}

// ---------------------------------------------------------------------------------------------
// The pinned fluids
// ---------------------------------------------------------------------------------------------

/// The committed fixture is the single source of truth for the pinned fluid data. If a constant in
/// `pinned.rs` is ever edited without regenerating the fixture — or vice versa — this fails, which
/// is the only automated link between the Rust constants and the OPM headers they were read from.
///
/// Every system is checked against the specification it was generated from: components, the full
/// interaction matrix, and the EOS variant. The SPE5 constants are compared to 1 ulp-scale
/// tolerances because they are the header's own expressions, evaluated in the header's order.
#[test]
fn comp_spec_pinned_fluids_match_the_c0_fixture() {
    let f = fixture::load();
    for system in &f.systems {
        let spec = fixture::spec_for(system);
        assert_eq!(spec.component_count(), system.num_components);
        for (c, theirs) in spec.components().iter().zip(&system.components) {
            assert_eq!(c.id, theirs.name, "{}: component order", system.system);
            let checks: [(&str, f64, f64); 5] = [
                (
                    "molar_mass_kg_per_mol",
                    c.molar_mass_kg_per_mol,
                    theirs.molar_mass_kg_per_mol,
                ),
                (
                    "critical_temperature_k",
                    c.critical_temperature_k,
                    theirs.critical_temperature_k,
                ),
                (
                    "critical_pressure_pa",
                    c.critical_pressure_pa,
                    theirs.critical_pressure_pa,
                ),
                (
                    "critical_volume_m3_per_kmol",
                    c.critical_volume_m3_per_kmol,
                    theirs.critical_volume_m3_per_kmol,
                ),
                ("acentric_factor", c.acentric_factor, theirs.acentric_factor),
            ];
            for (key, ours, theirs) in checks {
                assert!(
                    (ours - theirs).abs() <= 4.0 * f64::EPSILON * theirs.abs(),
                    "{} {}: {key} is {ours} in pinned.rs but {theirs} in the fixture",
                    system.system,
                    c.id
                );
            }
        }
        for i in 0..system.num_components {
            for j in 0..system.num_components {
                assert_eq!(
                    spec.interaction(i, j),
                    system.binary_interaction[i][j],
                    "{}: k[{i}][{j}]",
                    system.system
                );
            }
        }
        let eos = match spec.eos() {
            EosVariant::PengRobinson => "PR",
            EosVariant::PengRobinsonCorrected => "PRCORR",
        };
        assert_eq!(eos, system.eos, "{}: EOS variant", system.system);
    }
}

/// The SPE5 interactions, stated against the header rather than the fixture, so a regenerated
/// fixture that dropped them would not pass by agreeing with a pinned fluid that also lost them.
#[test]
fn comp_spec_spe5_interactions_are_the_headers() {
    let spec = pinned::spe5().unwrap();
    let ids: Vec<_> = spec.components().iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["C1", "C3", "C6", "C10", "C15", "C20"]);
    let k =
        |a: &str, b: &str| spec.interaction(spec.index_of(a).unwrap(), spec.index_of(b).unwrap());
    for heavy in ["C15", "C20"] {
        assert_eq!(k("C1", heavy), 0.05);
        assert_eq!(k(heavy, "C1"), 0.05);
        assert_eq!(k("C3", heavy), 0.005);
        assert_eq!(k(heavy, "C3"), 0.005);
    }
    let nonzero = (0..6)
        .flat_map(|i| (0..6).map(move |j| (i, j)))
        .filter(|&(i, j)| spec.interaction(i, j) != 0.0)
        .count();
    assert_eq!(nonzero, 8, "only C1/C3 against C15/C20 interact");
    assert_eq!(spec.eos(), EosVariant::PengRobinsonCorrected);
}

/// Temperature is part of each pinned fluid. The C0 fluids keep 423.15 K exactly; SPE5 is 160 °F
/// (Killough & Kossack 1987), which is 344.26 K.
#[test]
fn comp_spec_pinned_temperatures_belong_to_each_fluid() {
    assert_eq!(pinned::binary().unwrap().reservoir_temperature_k(), 423.15);
    assert_eq!(pinned::ternary().unwrap().reservoir_temperature_k(), 423.15);
    let t = pinned::spe5().unwrap().reservoir_temperature_k();
    assert_eq!(t, pinned::SPE5_RESERVOIR_TEMPERATURE_K);
    assert!((t - 344.261_111_111).abs() < 1e-8, "{t}");
}

/// Component order is load-bearing: the fixture's `z`, `x` and `y` arrays index straight into it.
#[test]
fn comp_spec_pinned_component_order_matches_opm() {
    let binary = pinned::binary().unwrap();
    assert_eq!(
        binary
            .components()
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        ["C1", "C10"]
    );

    let ternary = pinned::ternary().unwrap();
    assert_eq!(
        ternary
            .components()
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        ["CO2", "C1", "C10"],
        "order must match ThreeComponentFluidSystem's Comp0Idx..Comp2Idx"
    );
}

/// Surface conditions are an open decision (C5). If this ever fails, someone pinned them — which
/// is fine, but `COMPOSITIONAL_VALIDATION.md` §6 has to be updated in the same change.
#[test]
fn comp_spec_pinned_fluids_declare_surface_conditions_unpinned() {
    assert!(pinned::binary().unwrap().surface().is_none());
    assert!(pinned::ternary().unwrap().surface().is_none());
    assert!(pinned::spe5().unwrap().surface().is_none());
}

#[test]
fn comp_spec_c0_interactions_are_symmetric_zero() {
    for spec in [pinned::binary().unwrap(), pinned::ternary().unwrap()] {
        for i in 0..spec.component_count() {
            for j in 0..spec.component_count() {
                assert_eq!(spec.interaction(i, j), 0.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Specification validation
// ---------------------------------------------------------------------------------------------

#[test]
fn comp_spec_rejects_unsupported_component_counts() {
    assert_eq!(
        spec_of(vec![comp("A")]).unwrap_err(),
        FluidSpecError::UnsupportedComponentCount { count: 1 }
    );
    let too_many: Vec<_> = (0..=MAX_COMPONENTS)
        .map(|i| comp(&format!("C{i}")))
        .collect();
    let count = too_many.len();
    assert_eq!(
        spec_of(too_many).unwrap_err(),
        FluidSpecError::UnsupportedComponentCount { count }
    );
    // The bound itself is admitted.
    let at_bound: Vec<_> = (0..MAX_COMPONENTS)
        .map(|i| comp(&format!("C{i}")))
        .collect();
    assert!(spec_of(at_bound).is_ok());
    assert_eq!(
        MAX_COMPONENTS, 10,
        "#45 raised the bound to 10; change it deliberately"
    );
}

#[test]
fn comp_spec_rejects_duplicate_and_empty_ids() {
    assert_eq!(
        spec_of(vec![comp("C1"), comp("C1")]).unwrap_err(),
        FluidSpecError::DuplicateComponentId {
            id: "C1".to_string(),
            first: 0,
            second: 1
        }
    );
    assert_eq!(
        spec_of(vec![comp(""), comp("C10")]).unwrap_err(),
        FluidSpecError::EmptyComponentId { index: 0 }
    );
}

#[test]
fn comp_spec_rejects_invalid_component_properties() {
    for (field, mutate) in [
        (
            "molar_mass_kg_per_mol",
            (|c: &mut Component| c.molar_mass_kg_per_mol = 0.0) as fn(&mut Component),
        ),
        ("critical_temperature_k", |c: &mut Component| {
            c.critical_temperature_k = -1.0
        }),
        ("critical_pressure_pa", |c: &mut Component| {
            c.critical_pressure_pa = f64::NAN
        }),
        ("critical_volume_m3_per_kmol", |c: &mut Component| {
            c.critical_volume_m3_per_kmol = f64::INFINITY
        }),
        ("acentric_factor", |c: &mut Component| {
            c.acentric_factor = f64::NAN
        }),
    ] {
        let mut bad = comp("BAD");
        mutate(&mut bad);
        match spec_of(vec![bad, comp("C10")]).unwrap_err() {
            FluidSpecError::InvalidComponentProperty {
                field: f,
                index,
                id,
                ..
            } => {
                assert_eq!(f, field);
                assert_eq!(index, 0);
                assert_eq!(id, "BAD");
            }
            other => panic!("expected InvalidComponentProperty for {field}, got {other:?}"),
        }
    }
}

/// A negative acentric factor is physical (hydrogen), so only finiteness is checked. A universal
/// positivity range here would reject a real fluid; plan C1.1 puts model-specific ranges with the
/// correlation that consumes them.
#[test]
fn comp_spec_accepts_negative_acentric_factor() {
    let mut h2 = comp("H2");
    h2.acentric_factor = -0.216;
    assert!(spec_of(vec![h2, comp("C10")]).is_ok());
}

#[test]
fn comp_spec_rejects_bad_interaction_matrices() {
    let two = vec![comp("A"), comp("B")];
    let build = |k: Vec<Vec<f64>>| {
        FluidSpecification::new(
            two.clone(),
            k,
            EosVariant::PengRobinson,
            ViscosityModel::LohrenzBrayClark,
            TEST_TEMPERATURE_K,
            None,
        )
    };

    assert_eq!(
        build(vec![vec![0.0, 0.0]]).unwrap_err(),
        FluidSpecError::InteractionMatrixShape {
            expected: 2,
            rows: 1,
            cols: None
        }
    );
    assert_eq!(
        build(vec![vec![0.0, 0.0, 0.0], vec![0.0, 0.0, 0.0]]).unwrap_err(),
        FluidSpecError::InteractionMatrixShape {
            expected: 2,
            rows: 2,
            cols: Some(3)
        }
    );
    assert_eq!(
        build(vec![vec![0.0, 0.1], vec![0.2, 0.0]]).unwrap_err(),
        FluidSpecError::InteractionMatrixAsymmetric {
            i: 0,
            j: 1,
            k_ij: 0.1,
            k_ji: 0.2
        }
    );
    // Matched rather than compared: the payload carries the offending NaN, and NaN != NaN, so
    // `assert_eq!` on the whole error would fail however correct the code is.
    match build(vec![vec![0.0, f64::NAN], vec![f64::NAN, 0.0]]).unwrap_err() {
        FluidSpecError::InteractionCoefficientNotFinite { i, j, value } => {
            assert_eq!((i, j), (0, 1));
            assert!(value.is_nan());
        }
        other => panic!("expected InteractionCoefficientNotFinite, got {other:?}"),
    }
    // A symmetric nonzero matrix is accepted — the pinned fluids happen to be zero, but the type
    // is not restricted to that.
    assert!(build(vec![vec![0.0, 0.05], vec![0.05, 0.0]]).is_ok());
}

#[test]
fn comp_spec_rejects_non_absolute_temperatures_and_surface_pressure() {
    let two = vec![comp("A"), comp("B")];
    let build = |t: f64, s: Option<SurfaceConditions>| {
        FluidSpecification::new(
            two.clone(),
            vec![vec![0.0; 2]; 2],
            EosVariant::PengRobinson,
            ViscosityModel::LohrenzBrayClark,
            t,
            s,
        )
    };

    // 20 °C passed as Celsius instead of kelvin is the realistic version of this mistake, and it
    // is only caught at zero or below — which is exactly why `units::from_celsius` exists and why
    // nothing downstream accepts Celsius.
    assert_eq!(
        build(0.0, None).unwrap_err(),
        FluidSpecError::InvalidTemperature {
            field: "reservoir_temperature_k",
            value_k: 0.0
        }
    );
    assert_eq!(
        build(-40.0, None).unwrap_err(),
        FluidSpecError::InvalidTemperature {
            field: "reservoir_temperature_k",
            value_k: -40.0
        }
    );
    assert_eq!(
        build(
            423.15,
            Some(SurfaceConditions {
                pressure_pa: 101325.0,
                temperature_k: -1.0
            })
        )
        .unwrap_err(),
        FluidSpecError::InvalidTemperature {
            field: "surface_temperature_k",
            value_k: -1.0
        }
    );
    assert_eq!(
        build(
            423.15,
            Some(SurfaceConditions {
                pressure_pa: 0.0,
                temperature_k: 288.71
            })
        )
        .unwrap_err(),
        FluidSpecError::InvalidSurfacePressure { value_pa: 0.0 }
    );
    assert!(
        build(
            423.15,
            Some(SurfaceConditions {
                pressure_pa: 101325.0,
                temperature_k: 288.71
            })
        )
        .is_ok()
    );
}

// ---------------------------------------------------------------------------------------------
// Composition validation and the active-component policy
// ---------------------------------------------------------------------------------------------

#[test]
fn comp_spec_composition_rejects_invalid_input() {
    let spec = pinned::ternary().unwrap();

    assert_eq!(
        OverallComposition::new(&spec, vec![0.5, 0.5]).unwrap_err(),
        FluidSpecError::CompositionLength {
            expected: 3,
            actual: 2
        }
    );
    assert_eq!(
        OverallComposition::new(&spec, vec![0.5, -0.1, 0.6]).unwrap_err(),
        FluidSpecError::CompositionEntryInvalid {
            index: 1,
            value: -0.1
        }
    );
    match OverallComposition::new(&spec, vec![0.5, f64::NAN, 0.5]).unwrap_err() {
        FluidSpecError::CompositionEntryInvalid { index, value } => {
            assert_eq!(index, 1);
            assert!(value.is_nan());
        }
        other => panic!("expected CompositionEntryInvalid, got {other:?}"),
    }
    assert_eq!(
        OverallComposition::new(&spec, vec![0.0, 0.0, 0.0]).unwrap_err(),
        FluidSpecError::CompositionAllZero
    );
    match OverallComposition::new(&spec, vec![0.2, 0.5, 0.2]).unwrap_err() {
        FluidSpecError::CompositionSum { sum, .. } => assert!((sum - 0.9).abs() < 1e-12),
        other => panic!("expected CompositionSum, got {other:?}"),
    }
}

/// Roundoff at the last bit must pass; a materially wrong sum must not. Both directions matter:
/// a tolerance that only accepts exact sums rejects every composition that has been through a
/// text format.
#[test]
fn comp_spec_composition_sum_tolerance_is_two_sided() {
    let spec = pinned::ternary().unwrap();
    let eps = 1e-13;
    assert!(OverallComposition::new(&spec, vec![0.2 + eps, 0.5, 0.3]).is_ok());
    assert!(OverallComposition::new(&spec, vec![0.2 - eps, 0.5, 0.3]).is_ok());
    assert!(OverallComposition::new(&spec, vec![0.2 + 1e-6, 0.5, 0.3]).is_err());

    // An explicit looser tolerance is available, but it has to be asked for at the call site.
    assert!(
        OverallComposition::new_with_tolerance(&spec, vec![0.2 + 1e-6, 0.5, 0.3], 1e-5).is_ok()
    );
}

/// The entries are stored exactly as supplied. If construction renormalized, an exact zero would
/// become a tiny nonzero and the absent/present distinction would be destroyed.
#[test]
fn comp_spec_composition_is_not_renormalized() {
    let spec = pinned::ternary().unwrap();
    let z = vec![0.0, 0.6, 0.4];
    let c = OverallComposition::new(&spec, z.clone()).unwrap();
    assert_eq!(c.as_slice(), z.as_slice());
    assert_eq!(c.activity(0), ComponentActivity::Absent);
    assert_eq!(c.active_indices(), vec![1, 2]);
}

/// A trace component is present, not absent. This is the distinction C3's flash and C10's update
/// policy both depend on: a trace component stays in the equilibrium calculation, and an absent
/// one must still be allowed to become present when a well injects it.
#[test]
fn comp_spec_trace_component_is_present_not_absent() {
    let spec = pinned::ternary().unwrap();
    let c = OverallComposition::new(&spec, vec![1e-12, 0.6, 0.4 - 1e-12]).unwrap();
    assert_eq!(c.activity(0), ComponentActivity::Present);
    assert_eq!(c.active_indices(), vec![0, 1, 2]);

    // The C0 fixture's trace state, which is a real flashed reference: z_CO2 = 1e-6.
    let fixture_trace = OverallComposition::new(&spec, vec![1e-6, 0.6, 0.399999]).unwrap();
    assert_eq!(fixture_trace.activity(0), ComponentActivity::Present);
}

// ---------------------------------------------------------------------------------------------
// Permutation and serialization
// ---------------------------------------------------------------------------------------------

/// Reordering components must carry the interaction matrix. The specification exposes no way to do
/// one without the other, so this test checks that the supported operation is correct rather than
/// that an unsupported one is rejected — the latter is unrepresentable.
#[test]
fn comp_spec_permutation_carries_the_interaction_matrix() {
    let components = vec![comp("A"), comp("B"), comp("C")];
    let k = vec![
        vec![0.00, 0.10, 0.20],
        vec![0.10, 0.00, 0.30],
        vec![0.20, 0.30, 0.00],
    ];
    let spec = FluidSpecification::new(
        components,
        k,
        EosVariant::PengRobinson,
        ViscosityModel::LohrenzBrayClark,
        TEST_TEMPERATURE_K,
        None,
    )
    .unwrap();

    // Reverse the order: new 0 = old 2, new 1 = old 1, new 2 = old 0.
    let p = spec.permuted(&[2, 1, 0]).unwrap();
    assert_eq!(
        p.components()
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        ["C", "B", "A"]
    );
    // k(C,B) must still be 0.30 and k(C,A) still 0.20 — the coefficients followed their pairs.
    assert_eq!(p.interaction(0, 1), 0.30);
    assert_eq!(p.interaction(0, 2), 0.20);
    assert_eq!(p.interaction(1, 2), 0.10);

    // Every pair's coefficient is preserved under lookup by identity, which is the property that
    // actually matters and is independent of how the permutation was expressed.
    for (a, b) in [("A", "B"), ("A", "C"), ("B", "C")] {
        let before = spec.interaction(spec.index_of(a).unwrap(), spec.index_of(b).unwrap());
        let after = p.interaction(p.index_of(a).unwrap(), p.index_of(b).unwrap());
        assert_eq!(before, after, "k({a},{b}) changed under permutation");
    }

    // Applying the inverse returns the original.
    assert_eq!(p.permuted(&[2, 1, 0]).unwrap(), spec);
}

#[test]
fn comp_spec_permutation_rejects_non_bijections() {
    let spec = pinned::ternary().unwrap();
    for bad in [vec![0, 0, 1], vec![0, 1], vec![0, 1, 3], vec![0, 1, 2, 2]] {
        assert!(
            matches!(
                spec.permuted(&bad),
                Err(FluidSpecError::InvalidPermutation { .. })
            ),
            "{bad:?} should not be accepted as a permutation"
        );
    }
}

#[test]
fn comp_spec_composition_permutes_with_its_specification() {
    let spec = pinned::ternary().unwrap();
    let z = OverallComposition::new(&spec, vec![0.2, 0.5, 0.3]).unwrap();

    let order = [2, 0, 1];
    let spec_p = spec.permuted(&order).unwrap();
    let z_p = z.permuted(&order).unwrap();

    for c in spec.components() {
        let before = z.as_slice()[spec.index_of(&c.id).unwrap()];
        let after = z_p.as_slice()[spec_p.index_of(&c.id).unwrap()];
        assert_eq!(before, after, "z for {} moved under permutation", c.id);
    }
}

/// Order must survive a serialization roundtrip: a format that reordered components would silently
/// remap every composition read back through it.
#[test]
fn comp_spec_serde_roundtrip_preserves_order_and_values() {
    for spec in [pinned::binary().unwrap(), pinned::ternary().unwrap()] {
        let json = serde_json::to_string(&spec).expect("serialize");
        let back: FluidSpecification = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, spec);
        assert_eq!(
            back.components()
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<_>>(),
            spec.components()
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<_>>()
        );
    }
}

/// The EOS and viscosity tags are what make a serialized specification self-describing. A file
/// naming a variant this build does not have must fail to load rather than fall back to PR.
#[test]
fn comp_spec_serde_rejects_an_unknown_eos_variant() {
    let spec = pinned::binary().unwrap();
    let json = serde_json::to_string(&spec).unwrap();
    assert!(
        json.contains("PengRobinson"),
        "eos variant must be in the payload: {json}"
    );

    let tampered = json.replace("PengRobinson", "SoaveRedlichKwong");
    assert!(
        serde_json::from_str::<FluidSpecification>(&tampered).is_err(),
        "an unknown EOS tag must be rejected, not silently defaulted"
    );

    let tampered = json.replace("LohrenzBrayClark", "ConstantViscosity");
    assert!(
        serde_json::from_str::<FluidSpecification>(&tampered).is_err(),
        "an unknown viscosity tag must be rejected, not silently defaulted"
    );
}
