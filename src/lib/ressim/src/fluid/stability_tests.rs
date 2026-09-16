//! C3 stability contract tests (`comp_stability_*`).
//!
//! The fixture is a direct oracle here: `present_phase` is OPM's own verdict for each of the 47
//! flashed states, arrived at through the same Michelsen test. Agreeing with it on every state is
//! the strongest available check, because a stability test that is merely self-consistent can be
//! confidently wrong.

use super::fixture::{self, FixtureSystem};
use super::pinned;
use super::specification::FluidSpecification;
use super::stability::{StabilityVerdict, test_stability, wilson_k};

fn spec_for(system: &FixtureSystem) -> FluidSpecification {
    match system.num_components {
        2 => pinned::binary().unwrap(),
        3 => pinned::ternary().unwrap(),
        n => panic!("no pinned specification for {n} components"),
    }
}

/// Every flashed state's verdict, against OPM's.
#[test]
fn comp_stability_verdict_matches_the_fixture_on_every_state() {
    let f = fixture::load();
    let mut two_phase = 0;
    let mut single_phase = 0;
    let mut disagreements = Vec::new();

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            let verdict = test_stability(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap_or_else(|e| panic!("{}: stability test failed: {e}", state.id));

            let ours_unstable = matches!(verdict, StabilityVerdict::Unstable { .. });
            let theirs_unstable = state.is_two_phase();

            if ours_unstable != theirs_unstable {
                disagreements.push(format!(
                    "{}: we say {}, OPM says {}",
                    state.id,
                    if ours_unstable { "unstable" } else { "stable" },
                    state.present_phase
                ));
            }
            if theirs_unstable {
                two_phase += 1;
            } else {
                single_phase += 1;
            }
        }
    }

    assert_eq!(two_phase, 29, "the fixture's two-phase count changed");
    assert_eq!(single_phase, 18, "the fixture's single-phase count changed");
    assert!(
        disagreements.is_empty(),
        "stability verdicts disagree with OPM on {} of 47 states:\n{}",
        disagreements.len(),
        disagreements.join("\n")
    );
}

/// The converged verdict must not depend on the starting guess. If it did, a cell's phase state
/// would be a function of iteration history rather than of the fluid — and warm-starting from the
/// previous timestep, which C10 will do everywhere, would change physics.
#[test]
fn comp_stability_verdict_is_independent_of_the_starting_guess() {
    let f = fixture::load();

    for system in &f.systems {
        let spec = spec_for(system);
        let n = spec.component_count();
        for state in &system.states {
            let cold = test_stability(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap();

            // Three deliberately poor warm starts: the Wilson guess scaled up and down by an order
            // of magnitude, and a uniform K = 1 vector, which is the trivial solution itself.
            let wilson = wilson_k(&spec, state.pressure_pa, state.temperature_k);
            let seeds: [Vec<f64>; 3] = [
                wilson.iter().map(|k| k * 10.0).collect(),
                wilson.iter().map(|k| k / 10.0).collect(),
                vec![1.0 + 1e-3; n],
            ];

            for (which, seed) in seeds.iter().enumerate() {
                let warm = test_stability(
                    &spec,
                    state.pressure_pa,
                    state.temperature_k,
                    &state.z,
                    Some(seed),
                );
                // A poor seed may legitimately fail to converge; what it must never do is reach
                // the *opposite* verdict.
                if let Ok(warm) = warm {
                    let same = matches!(
                        (&cold, &warm),
                        (StabilityVerdict::Stable, StabilityVerdict::Stable)
                            | (
                                StabilityVerdict::Unstable { .. },
                                StabilityVerdict::Unstable { .. }
                            )
                    );
                    assert!(
                        same,
                        "{}: seed {which} changed the verdict from {:?} to {:?}",
                        state.id,
                        matches!(cold, StabilityVerdict::Stable),
                        matches!(warm, StabilityVerdict::Stable)
                    );
                }
            }
        }
    }
}

/// Wilson is a correlation, not an answer: check it reproduces the source formula and behaves
/// sensibly rather than trusting it.
#[test]
fn comp_stability_wilson_estimate_matches_its_source_formula() {
    let spec = pinned::ternary().unwrap();
    let p = 1.5e7;
    let t = 423.15;
    let k = wilson_k(&spec, p, t);

    for i in 0..spec.component_count() {
        let c = spec.component(i);
        let expected = (5.3727 * (1.0 + c.acentric_factor) * (1.0 - c.critical_temperature_k / t))
            .exp()
            * (c.critical_pressure_pa / p);
        assert!((k[i] - expected).abs() / expected < 1e-15, "{}", c.id);
    }

    // Methane is far above its critical temperature here and n-decane far below, so the light
    // component must partition into the vapour and the heavy one into the liquid.
    let c1 = spec.index_of("C1").unwrap();
    let c10 = spec.index_of("C10").unwrap();
    assert!(k[c1] > 1.0, "K for methane should exceed 1: {}", k[c1]);
    assert!(k[c10] < 1.0, "K for n-decane should be below 1: {}", k[c10]);
}

/// A trace component is present and must not change the verdict for its host mixture. The
/// fixture's `ternary_trace_co2` and the matching `binary_p150` are the same physical state to
/// within 1e-6 of CO2, and OPM calls both two-phase.
#[test]
fn comp_stability_a_trace_component_does_not_flip_the_verdict() {
    let ternary = pinned::ternary().unwrap();
    let binary = pinned::binary().unwrap();
    let p = 1.5e7;
    let t = 423.15;

    let with_trace = test_stability(&ternary, p, t, &[1e-6, 0.6, 0.399999], None).unwrap();
    let without = test_stability(&binary, p, t, &[0.6, 0.4], None).unwrap();

    assert!(matches!(with_trace, StabilityVerdict::Unstable { .. }));
    assert!(matches!(without, StabilityVerdict::Unstable { .. }));
}

/// An exactly zero component is material that is not there. The test must run and must reach the
/// same verdict as the equivalent smaller system — this is the case OPM's own flash cannot handle
/// (`docs/COMPOSITIONAL_VALIDATION.md` §4), so there is no external oracle and the cross-system
/// comparison is the check.
#[test]
fn comp_stability_handles_an_exactly_zero_component() {
    let ternary = pinned::ternary().unwrap();
    let binary = pinned::binary().unwrap();
    let t = 423.15;

    for p in [2.0e6, 5.0e6, 1.5e7, 2.0e7] {
        let with_zero = test_stability(&ternary, p, t, &[0.0, 0.6, 0.4], None);
        let reduced = test_stability(&binary, p, t, &[0.6, 0.4], None);

        match (&with_zero, &reduced) {
            (Ok(a), Ok(b)) => {
                let same = matches!(
                    (a, b),
                    (StabilityVerdict::Stable, StabilityVerdict::Stable)
                        | (
                            StabilityVerdict::Unstable { .. },
                            StabilityVerdict::Unstable { .. }
                        )
                );
                assert!(
                    same,
                    "p = {p}: a zero CO2 ternary and the C1/C10 binary disagree: {a:?} vs {b:?}"
                );
            }
            (Err(e), _) => panic!("p = {p}: zero-component feed failed: {e}"),
            (_, Err(e)) => panic!("p = {p}: binary feed failed: {e}"),
        }
    }
}

/// When the feed is unstable, the returned `K` must actually separate the components — a `K` of
/// all ones carries no information and would hand the flash the trivial solution.
#[test]
fn comp_stability_unstable_verdict_returns_a_usable_k_estimate() {
    let f = fixture::load();
    let mut checked = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            if !state.is_two_phase() {
                continue;
            }
            let StabilityVerdict::Unstable { k } = test_stability(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap() else {
                panic!("{}: expected an unstable verdict", state.id);
            };

            assert_eq!(k.len(), spec.component_count());
            for (i, &value) in k.iter().enumerate() {
                assert!(
                    value.is_finite() && value > 0.0,
                    "{}: K[{i}] = {value}",
                    state.id
                );
            }
            let spread: f64 = k.iter().map(|v| v.ln().abs()).fold(0.0, f64::max);
            assert!(
                spread > 1e-2,
                "{}: K = {k:?} is too close to the trivial solution to seed a flash",
                state.id
            );

            // The heaviest component must favour the liquid and the lightest the vapour. This is
            // physics, not a restatement of the algorithm.
            let heavy = spec.component_count() - 1;
            let light = spec.component_count() - 2;
            assert!(
                k[heavy] < k[light],
                "{}: K is not ordered by volatility: {k:?}",
                state.id
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 29);
}

#[test]
fn comp_stability_rejects_a_mismatched_composition_length() {
    let spec = pinned::ternary().unwrap();
    assert!(test_stability(&spec, 1.5e7, 423.15, &[0.5, 0.5], None).is_err());
}
