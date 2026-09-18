//! C5 contract tests (`comp_transport_*` and `comp_surface_*`).

use super::eos::PhaseBranch;
use super::fixture::{self, FixtureSystem};
use super::flash::{PhaseState, flash};
use super::pinned;
use super::specification::{FluidSpecification, SurfaceConditions};
use super::transport::{
    STANDARD_PRESSURE_PA, STANDARD_TEMPERATURE_K, SurfaceStream, TransportError, flash_viscosities,
    lbc_viscosity, pinned_surface_conditions, surface_separation,
};
use super::units;

fn spec_for(system: &FixtureSystem) -> FluidSpecification {
    match system.num_components {
        2 => pinned::binary().unwrap(),
        3 => pinned::ternary().unwrap(),
        n => panic!("no pinned specification for {n} components"),
    }
}

/// The pinned fluids with surface conditions attached, which is what the surface flash needs.
fn spec_with_surface(n: usize) -> FluidSpecification {
    let base = match n {
        2 => pinned::binary().unwrap(),
        3 => pinned::ternary().unwrap(),
        other => panic!("no pinned specification for {other} components"),
    };
    FluidSpecification::new(
        base.components().to_vec(),
        (0..n)
            .map(|i| (0..n).map(|j| base.interaction(i, j)).collect())
            .collect(),
        base.eos(),
        base.viscosity_model(),
        base.reservoir_temperature_k(),
        Some(pinned_surface_conditions()),
    )
    .unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

// ---------------------------------------------------------------------------------------------
// Viscosity
// ---------------------------------------------------------------------------------------------

/// LBC against the fixture, fed **OPM's own molar density**.
///
/// The correlation's only route to a gas constant is the density it is given, and OPM's `R` is the
/// superseded `8.314472` while this crate's is the exact SI value. Passing OPM's density isolates
/// the correlation from that difference, so this comparison measures the correlation and nothing
/// else. The size of the difference the SI density makes is measured separately below rather than
/// hidden in a tolerance here.
#[test]
fn comp_transport_lbc_matches_the_fixture_at_opms_own_density() {
    let f = fixture::load();
    let mut worst = (0.0f64, String::new());
    let mut checked = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            for (liquid, tag) in [(true, "L"), (false, "V")] {
                let Some(phase) = (if liquid { &state.liquid } else { &state.vapour }) else {
                    continue;
                };
                let x = state.composition_for(liquid);
                let ours = lbc_viscosity::<f64>(&spec, state.temperature_k, x, phase.molar_density)
                    .unwrap_or_else(|e| panic!("{}: {e}", state.id));

                let e = rel(ours, phase.viscosity);
                if e > worst.0 {
                    worst = (e, format!("{}/{tag}", state.id));
                }
                checked += 1;
            }
        }
    }

    assert!(checked >= 90, "compared only {checked} viscosities");
    assert!(
        worst.0 < 1e-13,
        "LBC disagrees with the fixture: worst relative error {:e} at {}",
        worst.0,
        worst.1
    );
}

/// How much the gas-constant difference actually moves viscosity.
///
/// Unlike density, this is **not** a clean ratio: LBC's dependence on the reduced density is a
/// quartic polynomial, so the 1.128e-6 shift in density propagates non-linearly. Measuring it here
/// means the number is recorded rather than assumed, and it is well inside any tolerance a
/// mobility calculation would care about.
#[test]
fn comp_transport_gas_constant_shift_moves_viscosity_by_a_measured_amount() {
    let f = fixture::load();
    let mut worst = 0.0f64;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            for liquid in [true, false] {
                let Some(phase) = (if liquid { &state.liquid } else { &state.vapour }) else {
                    continue;
                };
                let x = state.composition_for(liquid);
                let with_opm =
                    lbc_viscosity::<f64>(&spec, state.temperature_k, x, phase.molar_density)
                        .unwrap();
                let with_si = lbc_viscosity::<f64>(
                    &spec,
                    state.temperature_k,
                    x,
                    units::opm_density_to_si(phase.molar_density),
                )
                .unwrap();
                worst = worst.max(rel(with_si, with_opm));
            }
        }
    }

    assert!(
        worst < 1e-5,
        "the gas-constant shift moves viscosity by {worst:e}, more than recorded"
    );
    assert!(
        worst > 1e-9,
        "the shift now has no measurable effect on viscosity ({worst:e}); if the oracle's gas \
         constant was updated, the conversion machinery can be retired"
    );
}

/// Viscosity must be positive, finite, and higher in the liquid than the vapour. Independent of
/// the fixture: a correlation that got the phases the wrong way round would still reproduce a
/// reference that had the same error.
#[test]
fn comp_transport_liquid_is_more_viscous_than_vapour() {
    let f = fixture::load();
    let mut checked = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            let fs = flash(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap();
            let (mu_l, mu_v) = flash_viscosities(&spec, state.temperature_k, &fs).unwrap();

            match fs.phase_state {
                PhaseState::TwoPhase => {
                    let (l, v) = (mu_l.unwrap(), mu_v.unwrap());
                    assert!(l > 0.0 && l.is_finite(), "{}: mu_L = {l}", state.id);
                    assert!(v > 0.0 && v.is_finite(), "{}: mu_V = {v}", state.id);
                    assert!(
                        l > v,
                        "{}: liquid {l:e} is not more viscous than vapour {v:e}",
                        state.id
                    );
                    checked += 1;
                }
                PhaseState::SingleLiquid => {
                    assert!(mu_l.is_some() && mu_v.is_none(), "{}", state.id)
                }
                PhaseState::SingleVapour => {
                    assert!(mu_v.is_some() && mu_l.is_none(), "{}", state.id)
                }
            }
        }
    }
    assert_eq!(checked, 36);
}

/// An absent phase has no viscosity. Zero would be worse than nothing: a mobility calculation
/// would divide by it.
#[test]
fn comp_transport_absent_phase_has_no_viscosity() {
    let spec = pinned::binary().unwrap();
    let liquid_state = flash(&spec, 3.0e7, 423.15, &[0.6, 0.4], None).unwrap();
    assert_eq!(liquid_state.phase_state, PhaseState::SingleLiquid);
    let (l, v) = flash_viscosities(&spec, 423.15, &liquid_state).unwrap();
    assert!(l.is_some());
    assert_eq!(v, None);
}

/// Viscosity in reservoir units, through the named conversion rather than a factor at the call
/// site. The fixture's liquid viscosities are around 1e-4 Pa·s, which is 0.1 cP — a plausible
/// light-oil value, and a useful sanity anchor on the whole chain.
#[test]
fn comp_transport_viscosity_converts_to_plausible_centipoise() {
    let spec = pinned::ternary().unwrap();
    let state = flash(&spec, 1.5e7, 423.15, &[0.2, 0.5, 0.3], None).unwrap();
    let (mu_l, mu_v) = flash_viscosities(&spec, 423.15, &state).unwrap();

    let cp_l = units::pa_s_to_cp(mu_l.unwrap());
    let cp_v = units::pa_s_to_cp(mu_v.unwrap());
    assert!(
        cp_l > 0.01 && cp_l < 5.0,
        "liquid viscosity {cp_l} cP is not a plausible oil"
    );
    assert!(
        cp_v > 0.001 && cp_v < 0.1,
        "vapour viscosity {cp_v} cP is not a plausible gas"
    );
}

#[test]
fn comp_transport_rejects_invalid_input() {
    let spec = pinned::binary().unwrap();
    assert!(matches!(
        lbc_viscosity::<f64>(&spec, 423.15, &[1.0], 5000.0),
        Err(TransportError::InvalidComposition { .. })
    ));
    for bad in [0.0, -1.0, f64::NAN] {
        assert!(matches!(
            lbc_viscosity::<f64>(&spec, 423.15, &[0.6, 0.4], bad),
            Err(TransportError::InvalidMolarDensity { .. })
        ));
    }
}

// ---------------------------------------------------------------------------------------------
// Saturations from a mole split
// ---------------------------------------------------------------------------------------------

/// Saturation is a volume fraction reconstructed from phase mole amounts and molar volumes. The
/// reconstruction must be exact, and it must not equal `beta`.
#[test]
fn comp_transport_saturations_reconstruct_from_moles_and_molar_volumes() {
    let f = fixture::load();
    let mut worst_gap = 0.0f64;
    let mut checked = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            let fs = flash(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap();
            if fs.phase_state != PhaseState::TwoPhase {
                continue;
            }
            checked += 1;

            // Take one mole of mixture, split it, and measure the two volumes directly.
            let vapour_moles = fs.beta;
            let liquid_moles = 1.0 - fs.beta;
            let v_l = liquid_moles / fs.liquid.as_ref().unwrap().molar_density;
            let v_v = vapour_moles / fs.vapour.as_ref().unwrap().molar_density;

            let s_v = v_v / (v_l + v_v);
            assert!(
                (s_v - fs.vapour_saturation()).abs() < 1e-14,
                "{}: independent volume split {s_v} vs reported {}",
                state.id,
                fs.vapour_saturation()
            );
            assert!(
                (v_l + v_v - fs.mixture_molar_volume()).abs() / fs.mixture_molar_volume() < 1e-14,
                "{}: mixture molar volume does not match the phase volumes",
                state.id
            );
            worst_gap = worst_gap.max((s_v - fs.beta).abs());
        }
    }

    assert_eq!(checked, 36);
    assert!(
        worst_gap > 0.3,
        "saturation and beta never differ by much ({worst_gap}); the distinction this test exists \
         for is not being exercised"
    );
}

// ---------------------------------------------------------------------------------------------
// Surface separation
// ---------------------------------------------------------------------------------------------

/// The surface conditions C5 pins, against their source. If these change, so does every surface
/// volume the model ever reports, so the values are asserted rather than assumed.
#[test]
fn comp_surface_conditions_match_their_opm_source() {
    // opm/input/eclipse/Units/Units.hpp: atm = 101325 Pa.
    assert_eq!(STANDARD_PRESSURE_PA, 101_325.0);
    // CompositionalConfig.hpp: standard_temperature = 288.71 K, for compositional runs.
    assert_eq!(STANDARD_TEMPERATURE_K, 288.71);

    let s = pinned_surface_conditions();
    assert_eq!(s.pressure_pa, STANDARD_PRESSURE_PA);
    assert_eq!(s.temperature_k, STANDARD_TEMPERATURE_K);
    // 15.56 °C.
    assert!((s.temperature_k - units::from_celsius(15.56)).abs() < 1e-9);
}

/// Component moles are conserved exactly. The separation redistributes material between two
/// phases; it creates none and destroys none.
#[test]
fn comp_surface_separation_conserves_every_component() {
    for n in [2usize, 3usize] {
        let spec = spec_with_surface(n);
        let streams: Vec<Vec<f64>> = if n == 2 {
            vec![vec![600.0, 400.0], vec![10.0, 990.0], vec![990.0, 10.0]]
        } else {
            vec![
                vec![200.0, 500.0, 300.0],
                vec![1.0, 1.0, 998.0],
                vec![500.0, 490.0, 10.0],
            ]
        };

        for stream in streams {
            let out = surface_separation(&spec, &stream).unwrap();
            let total: f64 = stream.iter().sum();
            assert!((out.total_moles - total).abs() < 1e-12 * total);
            assert!(
                (out.liquid_moles + out.vapour_moles - total).abs() < 1e-9 * total,
                "phase moles do not sum to the stream: {out:?}"
            );
            for i in 0..n {
                let recovered = out.liquid_component_moles[i] + out.vapour_component_moles[i];
                assert!(
                    (recovered - stream[i]).abs() <= 1e-9 * total,
                    "component {i}: {recovered} recovered from {}",
                    stream[i]
                );
            }
        }
    }
}

/// A rich stream must actually split at surface conditions, and the volumes must follow from the
/// phase molar densities rather than from the mole split.
#[test]
fn comp_surface_separation_produces_both_phases_from_a_rich_stream() {
    let spec = spec_with_surface(3);
    let out = surface_separation(&spec, &[200.0, 500.0, 300.0]).unwrap();

    assert_eq!(out.phase_state, PhaseState::TwoPhase, "{out:?}");
    assert!(out.liquid_volume > 0.0 && out.vapour_volume > 0.0);

    // At 1 atm the surface gas is nearly ideal, so its volume per mole is close to R T / p.
    let ideal_molar_volume =
        units::GAS_CONSTANT_J_PER_MOL_K * STANDARD_TEMPERATURE_K / STANDARD_PRESSURE_PA;
    let actual = out.vapour_volume / out.vapour_moles;
    assert!(
        rel(actual, ideal_molar_volume) < 0.02,
        "surface gas molar volume {actual} is not near the ideal {ideal_molar_volume}"
    );

    // And the liquid is three orders denser, so the gas dominates the volume even though it does
    // not dominate the moles.
    assert!(
        out.vapour_volume > 100.0 * out.liquid_volume,
        "surface gas should dominate the volume: {} vs {}",
        out.vapour_volume,
        out.liquid_volume
    );

    let ratio = out.surface_gas_liquid_ratio().unwrap();
    assert!(
        ratio > 100.0 && ratio.is_finite(),
        "surface gas/liquid ratio {ratio}"
    );
}

/// A stream that is single phase at surface conditions gets one volume and a `None` ratio rather
/// than a division by zero.
#[test]
fn comp_surface_separation_handles_a_single_phase_stream() {
    let spec = spec_with_surface(2);
    // Essentially pure methane: all gas at 1 atm and 288.71 K.
    let out = surface_separation(&spec, &[1000.0, 1e-9]).unwrap();
    assert_eq!(out.phase_state, PhaseState::SingleVapour, "{out:?}");
    assert_eq!(out.liquid_volume, 0.0);
    assert!(out.vapour_volume > 0.0);
    assert_eq!(out.surface_gas_liquid_ratio(), None);
}

/// Zero flow has an answer. A shut-in well must not divide a zero rate by a molar density that
/// was never computed.
#[test]
fn comp_surface_separation_handles_zero_flow() {
    let spec = spec_with_surface(3);
    let out = surface_separation(&spec, &[0.0, 0.0, 0.0]).unwrap();
    assert_eq!(out, SurfaceStream::zero(3));
    assert_eq!(out.total_moles, 0.0);
    assert_eq!(out.liquid_volume, 0.0);
    assert_eq!(out.vapour_volume, 0.0);
    assert_eq!(out.surface_gas_liquid_ratio(), None);
}

/// Surface volumes are undefined without surface conditions, and saying so is better than
/// inventing a standard state. The pinned fluids ship without them by design, since C0 recorded
/// the decision as open and C5 is what closes it.
#[test]
fn comp_surface_separation_requires_pinned_conditions() {
    let without = pinned::ternary().unwrap();
    assert!(without.surface().is_none());
    assert!(matches!(
        surface_separation(&without, &[0.2, 0.5, 0.3]),
        Err(TransportError::SurfaceConditionsNotPinned)
    ));
}

#[test]
fn comp_surface_separation_rejects_an_invalid_stream() {
    let spec = spec_with_surface(3);
    assert!(matches!(
        surface_separation(&spec, &[1.0, 1.0]),
        Err(TransportError::InvalidComposition { .. })
    ));
    assert!(matches!(
        surface_separation(&spec, &[1.0, -1.0, 1.0]),
        Err(TransportError::InvalidComposition { .. })
    ));
    assert!(matches!(
        surface_separation(&spec, &[1.0, f64::NAN, 1.0]),
        Err(TransportError::InvalidComposition { .. })
    ));
}

/// Scaling the stream scales the volumes proportionally: the separation depends on composition,
/// not on how much of it there is. A calculation that leaked an absolute rate into the flash
/// would fail this.
#[test]
fn comp_surface_separation_is_linear_in_the_stream_size() {
    let spec = spec_with_surface(3);
    let base = surface_separation(&spec, &[200.0, 500.0, 300.0]).unwrap();
    let scaled = surface_separation(&spec, &[2000.0, 5000.0, 3000.0]).unwrap();

    assert_eq!(base.phase_state, scaled.phase_state);
    for (a, b) in [
        (base.liquid_volume, scaled.liquid_volume),
        (base.vapour_volume, scaled.vapour_volume),
        (base.liquid_moles, scaled.liquid_moles),
        (base.vapour_moles, scaled.vapour_moles),
    ] {
        assert!(rel(b, 10.0 * a) < 1e-12, "{b} is not ten times {a}");
    }
}

/// Reservoir and surface conditions must give different answers. If the surface flash were
/// accidentally run at reservoir conditions this would pass everything above and still be wrong.
#[test]
fn comp_surface_separation_is_evaluated_at_surface_not_reservoir_conditions() {
    let spec = spec_with_surface(3);
    let stream = [200.0, 500.0, 300.0];
    let surface = surface_separation(&spec, &stream).unwrap();

    let z: Vec<f64> = stream.iter().map(|m| m / 1000.0).collect();
    let reservoir = flash(&spec, 1.5e7, spec.reservoir_temperature_k(), &z, None).unwrap();

    assert!(
        (surface.vapour_moles / surface.total_moles - reservoir.beta).abs() > 0.1,
        "the surface split is suspiciously close to the reservoir one"
    );
}

/// Surface conditions are validated like any other input: the specification rejects a
/// non-physical pair rather than carrying it into the flash.
#[test]
fn comp_surface_conditions_are_validated_by_the_specification() {
    let base = pinned::binary().unwrap();
    let build = |s: SurfaceConditions| {
        FluidSpecification::new(
            base.components().to_vec(),
            vec![vec![0.0; 2]; 2],
            base.eos(),
            base.viscosity_model(),
            base.reservoir_temperature_k(),
            Some(s),
        )
    };
    assert!(
        build(SurfaceConditions {
            pressure_pa: 0.0,
            temperature_k: 288.71
        })
        .is_err()
    );
    assert!(
        build(SurfaceConditions {
            pressure_pa: 101325.0,
            temperature_k: 0.0
        })
        .is_err()
    );
    assert!(build(pinned_surface_conditions()).is_ok());
}

/// Viscosity derivatives, which C8 onward need for mobility. Checked against finite differences
/// rather than the fixture, because the fixture's `dviscosity_du` carries OPM's gas constant
/// through a non-linear path and is therefore not an exact oracle for this crate's value.
#[test]
fn comp_transport_viscosity_derivative_matches_finite_differences() {
    use crate::ad::Ad;

    let spec = pinned::binary().unwrap();
    let t = 423.15;
    let x = [0.6, 0.4];
    let c = 6000.0;

    // One AD slot: molar density.
    let x_ad = [Ad::<1>::constant(x[0]), Ad::<1>::constant(x[1])];
    let mu = lbc_viscosity::<Ad<1>>(&spec, t, &x_ad, Ad::variable(c, 0)).unwrap();

    let h = 1.0;
    let up = lbc_viscosity::<f64>(&spec, t, &x, c + h).unwrap();
    let down = lbc_viscosity::<f64>(&spec, t, &x, c - h).unwrap();
    let fd = (up - down) / (2.0 * h);

    assert!(
        rel(mu.d(0), fd) < 1e-6,
        "d(mu)/d(c): analytic {:e} vs FD {fd:e}",
        mu.d(0)
    );
    assert!(mu.d(0) > 0.0, "viscosity must increase with density");

    // And in composition: two slots, moving C1 up and C10 down so the sum stays one.
    let x2 = [
        Ad::<2>::variable(x[0], 0),
        Ad::<2>::seeded(x[1], [-1.0, 0.0]),
    ];
    let mu2 = lbc_viscosity::<Ad<2>>(&spec, t, &x2, Ad::constant(c)).unwrap();
    let hz = 1e-6;
    let up = lbc_viscosity::<f64>(&spec, t, &[x[0] + hz, x[1] - hz], c).unwrap();
    let down = lbc_viscosity::<f64>(&spec, t, &[x[0] - hz, x[1] + hz], c).unwrap();
    let fd = (up - down) / (2.0 * hz);
    assert!(
        rel(mu2.d(0), fd) < 1e-6,
        "d(mu)/d(x_C1): analytic {:e} vs FD {fd:e}",
        mu2.d(0)
    );
}

/// The branch on reduced temperature is a constant in an isothermal model: for the pinned fluids
/// at 423.15 K, methane is above `T_r = 1.5` and n-decane below it, so both sides of LBC's
/// piecewise dilute-gas term are exercised and neither is a moving target.
#[test]
fn comp_transport_both_dilute_gas_branches_are_exercised() {
    let spec = pinned::binary().unwrap();
    let t = spec.reservoir_temperature_k();
    let t_r_c1 = t / spec.component(0).critical_temperature_k;
    let t_r_c10 = t / spec.component(1).critical_temperature_k;
    assert!(
        t_r_c1 > 1.5,
        "methane T_r = {t_r_c1} should take the high branch"
    );
    assert!(
        t_r_c10 <= 1.5,
        "n-decane T_r = {t_r_c10} should take the low branch"
    );
}

/// An exactly zero component must not break the correlation: it contributes nothing to any of
/// LBC's sums and must leave the result equal to the smaller system's.
#[test]
fn comp_transport_lbc_ignores_an_absent_component() {
    let binary = pinned::binary().unwrap();
    let ternary = pinned::ternary().unwrap();
    let t = 423.15;
    let c = 6000.0;

    let mu_binary = lbc_viscosity::<f64>(&binary, t, &[0.6, 0.4], c).unwrap();
    let mu_ternary = lbc_viscosity::<f64>(&ternary, t, &[0.0, 0.6, 0.4], c).unwrap();
    assert!(
        rel(mu_ternary, mu_binary) < 1e-14,
        "a zero CO2 component changed the viscosity: {mu_ternary} vs {mu_binary}"
    );
}

/// Both EOS branches feed the viscosity, and the vapour branch at high density must not silently
/// reuse the liquid one. Guards against a copy-paste error the fixture comparison would catch
/// only if the fixture had the same error.
#[test]
fn comp_transport_uses_each_phases_own_density() {
    let spec = pinned::binary().unwrap();
    let t = 423.15;
    let state = flash(&spec, 1.0e7, t, &[0.6, 0.4], None).unwrap();
    assert_eq!(state.phase_state, PhaseState::TwoPhase);

    let liquid = state.liquid.as_ref().unwrap();
    let vapour = state.vapour.as_ref().unwrap();
    assert_eq!(liquid.branch, PhaseBranch::Liquid);
    assert_eq!(vapour.branch, PhaseBranch::Vapour);

    let (mu_l, mu_v) = flash_viscosities(&spec, t, &state).unwrap();
    // Recomputing with the phases swapped must give different numbers.
    let swapped_l = lbc_viscosity::<f64>(&spec, t, &state.x, vapour.molar_density).unwrap();
    assert!(
        rel(swapped_l, mu_l.unwrap()) > 1e-3,
        "the liquid viscosity does not depend on the liquid's own density"
    );
    assert!(mu_v.unwrap() < mu_l.unwrap());
}
