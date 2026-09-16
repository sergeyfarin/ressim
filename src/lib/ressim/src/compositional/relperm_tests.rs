//! Relative permeability model tests (`comp_relperm_*`).

use super::relperm::{
    CoreyParameters, RelPermError, RelativePermeabilityModel, RelativePermeabilityTable,
};
use crate::ad::Ad;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

/// `Linear` is the verification model, and it says so. A scenario-admission check can refuse a
/// case still running on it rather than relying on someone noticing.
#[test]
fn comp_relperm_linear_is_declared_verification_only() {
    let linear = RelativePermeabilityModel::Linear;
    assert!(linear.is_verification_only());
    assert_eq!(linear.name(), "linear");

    let corey = RelativePermeabilityModel::Corey(
        CoreyParameters::new(0.15, 0.05, 2.0, 3.0, 0.8, 0.6).unwrap(),
    );
    assert!(!corey.is_verification_only());
    assert_eq!(corey.name(), "corey");

    let table = RelativePermeabilityModel::Tabulated(
        RelativePermeabilityTable::new(vec![0.0, 1.0], vec![0.0, 1.0], vec![1.0, 0.0]).unwrap(),
    );
    assert!(!table.is_verification_only());
    assert_eq!(table.name(), "tabulated");
}

/// Straight lines, exactly: `kr_L = S_L` and `kr_V = 1 - S_L`, with no residual saturations and no
/// endpoint scaling. Both curves keyed on the liquid saturation.
#[test]
fn comp_relperm_linear_is_exactly_straight() {
    let m = RelativePermeabilityModel::Linear;
    for s in [0.0, 0.25, 0.5, 0.75, 1.0] {
        assert_eq!(m.kr_liquid::<f64>(s), s);
        assert_eq!(m.kr_vapour::<f64>(s), 1.0 - s);
        assert_eq!(m.kr::<f64>(s, true), m.kr_liquid::<f64>(s));
        assert_eq!(m.kr::<f64>(s, false), m.kr_vapour::<f64>(s));
        // The two sum to one everywhere, which is the property that makes them add no mobility
        // structure of their own.
        assert_eq!(m.kr_liquid::<f64>(s) + m.kr_vapour::<f64>(s), 1.0);
    }
}

/// Corey carries no defaults: every parameter is a required argument. A curve with made-up
/// exponents is made-up data wearing a respectable name, so there is nowhere for one to hide.
#[test]
fn comp_relperm_corey_has_no_defaults_and_validates_its_parameters() {
    // Valid.
    assert!(CoreyParameters::new(0.15, 0.05, 2.0, 3.0, 0.8, 0.6).is_ok());

    // Out of range, each field in turn.
    assert!(matches!(
        CoreyParameters::new(-0.1, 0.05, 2.0, 3.0, 0.8, 0.6),
        Err(RelPermError::InvalidParameter {
            field: "liquid_residual",
            ..
        })
    ));
    assert!(matches!(
        CoreyParameters::new(0.1, 1.5, 2.0, 3.0, 0.8, 0.6),
        Err(RelPermError::InvalidParameter {
            field: "vapour_residual",
            ..
        })
    ));
    // An exponent below one gives a curve convex at the endpoint, which is a transcription error
    // far more often than it is a fluid.
    assert!(matches!(
        CoreyParameters::new(0.1, 0.05, 0.5, 3.0, 0.8, 0.6),
        Err(RelPermError::InvalidParameter {
            field: "liquid_exponent",
            ..
        })
    ));
    assert!(matches!(
        CoreyParameters::new(0.1, 0.05, 2.0, f64::NAN, 0.8, 0.6),
        Err(RelPermError::InvalidParameter {
            field: "vapour_exponent",
            ..
        })
    ));
    assert!(matches!(
        CoreyParameters::new(0.1, 0.05, 2.0, 3.0, 1.2, 0.6),
        Err(RelPermError::InvalidParameter {
            field: "liquid_endpoint",
            ..
        })
    ));

    // Residuals that leave nothing mobile.
    assert!(matches!(
        CoreyParameters::new(0.6, 0.5, 2.0, 3.0, 0.8, 0.6),
        Err(RelPermError::NoMobileRange { .. })
    ));
}

/// Corey behaves as a Corey curve: zero below the residual, the endpoint at full mobility,
/// monotone in between, and matching the closed form.
#[test]
fn comp_relperm_corey_matches_its_closed_form() {
    let p = CoreyParameters::new(0.2, 0.1, 2.0, 3.0, 0.9, 0.7).unwrap();
    let m = RelativePermeabilityModel::Corey(p);

    // Immobile at and below the residual. Exactly zero below it, where the normalization clamps;
    // at the endpoint itself the value is zero to roundoff rather than bit-exactly, because
    // (1 - S_Lr - S_Vr) is not representable and S_e lands an ulp short of its limit.
    assert!(m.kr_liquid::<f64>(0.2).abs() < 1e-30);
    assert_eq!(m.kr_liquid::<f64>(0.1), 0.0);
    // At the other end the vapour is immobile.
    assert!(m.kr_vapour::<f64>(0.9).abs() < 1e-30);
    assert_eq!(m.kr_vapour::<f64>(1.0), 0.0);
    // Endpoints.
    assert!(rel(m.kr_liquid::<f64>(0.9), 0.9) < 1e-12);
    assert!(rel(m.kr_vapour::<f64>(0.2), 0.7) < 1e-12);

    // The closed form in between.
    for s in [0.3, 0.45, 0.6, 0.8] {
        let s_e = (s - 0.2) / (1.0 - 0.2 - 0.1);
        assert!(
            rel(m.kr_liquid::<f64>(s), 0.9 * s_e.powf(2.0)) < 1e-12,
            "s = {s}"
        );
        assert!(
            rel(m.kr_vapour::<f64>(s), 0.7 * (1.0 - s_e).powf(3.0)) < 1e-12,
            "s = {s}"
        );
    }

    // Monotone, in opposite directions.
    let mut previous_l = -1.0;
    let mut previous_v = f64::INFINITY;
    for step in 0..=20 {
        let s = step as f64 / 20.0;
        let l = m.kr_liquid::<f64>(s);
        let v = m.kr_vapour::<f64>(s);
        assert!(l >= previous_l, "kr_L decreased at s = {s}");
        assert!(v <= previous_v, "kr_V increased at s = {s}");
        assert!((0.0..=1.0).contains(&l) && (0.0..=1.0).contains(&v));
        previous_l = l;
        previous_v = v;
    }
}

/// A table is validated rather than trusted, because a curve from a deck is exactly the kind of
/// input that arrives subtly malformed.
#[test]
fn comp_relperm_table_is_validated() {
    assert!(RelativePermeabilityTable::new(vec![0.0, 1.0], vec![0.0, 1.0], vec![1.0, 0.0]).is_ok());

    // Too few rows, or mismatched columns.
    assert!(matches!(
        RelativePermeabilityTable::new(vec![0.5], vec![0.5], vec![0.5]),
        Err(RelPermError::TableShape { .. })
    ));
    assert!(matches!(
        RelativePermeabilityTable::new(vec![0.0, 1.0], vec![0.0], vec![1.0, 0.0]),
        Err(RelPermError::TableShape { .. })
    ));
    // Not strictly increasing.
    assert!(matches!(
        RelativePermeabilityTable::new(vec![0.0, 0.5, 0.5], vec![0.0; 3], vec![0.0; 3]),
        Err(RelPermError::TableNotIncreasing { index: 2, .. })
    ));
    assert!(matches!(
        RelativePermeabilityTable::new(vec![0.0, 0.6, 0.4], vec![0.0; 3], vec![0.0; 3]),
        Err(RelPermError::TableNotIncreasing { .. })
    ));
    // Negative or non-finite entries.
    assert!(matches!(
        RelativePermeabilityTable::new(vec![0.0, 1.0], vec![0.0, -0.1], vec![1.0, 0.0]),
        Err(RelPermError::TableEntry {
            column: "kr_liquid",
            ..
        })
    ));
    assert!(matches!(
        RelativePermeabilityTable::new(vec![0.0, 1.0], vec![0.0, 1.0], vec![f64::NAN, 0.0]),
        Err(RelPermError::TableEntry {
            column: "kr_vapour",
            ..
        })
    ));
}

/// Interpolation hits the tabulated points exactly and is linear between them, and it **clamps**
/// outside the table rather than extrapolating — an extrapolated relative permeability is a number
/// the table's author never stood behind, and can easily go negative.
#[test]
fn comp_relperm_table_interpolates_and_clamps() {
    let t = RelativePermeabilityTable::new(
        vec![0.2, 0.5, 0.8],
        vec![0.0, 0.3, 0.9],
        vec![0.8, 0.2, 0.0],
    )
    .unwrap();
    let m = RelativePermeabilityModel::Tabulated(t);

    // Exact at the nodes.
    for (s, l, v) in [(0.2, 0.0, 0.8), (0.5, 0.3, 0.2), (0.8, 0.9, 0.0)] {
        assert!((m.kr_liquid::<f64>(s) - l).abs() < 1e-12, "s = {s}");
        assert!((m.kr_vapour::<f64>(s) - v).abs() < 1e-12, "s = {s}");
    }
    // Linear in between.
    assert!((m.kr_liquid::<f64>(0.35) - 0.15).abs() < 1e-12);
    assert!((m.kr_vapour::<f64>(0.65) - 0.1).abs() < 1e-12);

    // Clamped outside, not extrapolated — which would go negative here.
    assert_eq!(m.kr_liquid::<f64>(0.0), 0.0);
    assert_eq!(m.kr_liquid::<f64>(-1.0), 0.0);
    assert_eq!(m.kr_vapour::<f64>(1.0), 0.0);
    assert_eq!(m.kr_vapour::<f64>(2.0), 0.0);
    assert_eq!(m.kr_liquid::<f64>(1.0), 0.9);
}

/// Every model is differentiable, because the flux and well Jacobians evaluate them with AD. The
/// derivatives are checked against finite differences away from the non-smooth points.
#[test]
fn comp_relperm_models_carry_derivatives() {
    let models = [
        RelativePermeabilityModel::Linear,
        RelativePermeabilityModel::Corey(
            CoreyParameters::new(0.2, 0.1, 2.0, 3.0, 0.9, 0.7).unwrap(),
        ),
        RelativePermeabilityModel::Tabulated(
            RelativePermeabilityTable::new(
                vec![0.2, 0.5, 0.8],
                vec![0.0, 0.3, 0.9],
                vec![0.8, 0.2, 0.0],
            )
            .unwrap(),
        ),
    ];

    for m in &models {
        // 0.35 and 0.65 sit strictly inside one table interval and inside Corey's mobile range.
        for s in [0.35, 0.65] {
            let ad = m.kr_liquid::<Ad<1>>(Ad::variable(s, 0));
            let h = 1e-7;
            let fd = (m.kr_liquid::<f64>(s + h) - m.kr_liquid::<f64>(s - h)) / (2.0 * h);
            assert!(
                (ad.d(0) - fd).abs() / fd.abs().max(1e-3) < 1e-5,
                "{}: d(kr_L)/dS at {s} is {} but FD gives {fd}",
                m.name(),
                ad.d(0)
            );
            assert!((ad.value() - m.kr_liquid::<f64>(s)).abs() < 1e-15);
        }
    }
}

/// `Corey` with unit exponents, no residuals and unit endpoints *is* the linear model. A useful
/// consistency check between the two branches, and it confirms Linear is not doing anything
/// special beyond being the degenerate Corey case.
#[test]
fn comp_relperm_corey_reduces_to_linear_at_unit_parameters() {
    let linear = RelativePermeabilityModel::Linear;
    let degenerate = RelativePermeabilityModel::Corey(
        CoreyParameters::new(0.0, 0.0, 1.0, 1.0, 1.0, 1.0).unwrap(),
    );
    for step in 0..=20 {
        let s = step as f64 / 20.0;
        assert!((degenerate.kr_liquid::<f64>(s) - linear.kr_liquid::<f64>(s)).abs() < 1e-15);
        assert!((degenerate.kr_vapour::<f64>(s) - linear.kr_vapour::<f64>(s)).abs() < 1e-15);
    }
}
