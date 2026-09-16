//! C3 flash contract tests (`comp_flash_*` and `comp_rr_*`).

use super::eos::PhaseBranch;
use super::fixture::{self, FixtureSystem};
use super::flash::{
    FlashError, PhaseState, flash, li_pseudo_critical_temperature, solve_rachford_rice,
};
use super::pinned;
use super::specification::FluidSpecification;
use super::stability::wilson_k;

fn spec_for(system: &FixtureSystem) -> FluidSpecification {
    match system.num_components {
        2 => pinned::binary().unwrap(),
        3 => pinned::ternary().unwrap(),
        n => panic!("no pinned specification for {n} components"),
    }
}

// ---------------------------------------------------------------------------------------------
// Rachford–Rice
// ---------------------------------------------------------------------------------------------

/// `g` is monotonically decreasing in `beta`, which is the property the bracketed solve depends on.
/// Asserted rather than assumed, across a wide range of `K` vectors.
#[test]
fn comp_rr_g_is_monotonically_decreasing() {
    let cases: [(Vec<f64>, Vec<f64>); 3] = [
        (vec![3.0, 0.2], vec![0.6, 0.4]),
        (vec![10.0, 2.0, 0.05], vec![0.2, 0.5, 0.3]),
        (vec![1.5, 0.9], vec![0.5, 0.5]),
    ];
    for (k, z) in cases {
        let g = |beta: f64| -> f64 {
            (0..k.len())
                .map(|i| z[i] * (k[i] - 1.0) / (1.0 + beta * (k[i] - 1.0)))
                .sum()
        };
        let mut previous = g(0.0);
        for step in 1..=100 {
            let beta = step as f64 / 100.0;
            let current = g(beta);
            assert!(
                current < previous,
                "g increased at beta = {beta} for K = {k:?}"
            );
            previous = current;
        }
    }
}

/// The recovered `beta` must satisfy the equation it solves, not merely lie in range.
#[test]
fn comp_rr_root_satisfies_the_equation() {
    let k = vec![10.0, 2.0, 0.05];
    let z = vec![0.2, 0.5, 0.3];
    let beta = solve_rachford_rice(&k, &z).unwrap();
    assert!(beta > 0.0 && beta < 1.0);

    let g: f64 = (0..k.len())
        .map(|i| z[i] * (k[i] - 1.0) / (1.0 + beta * (k[i] - 1.0)))
        .sum();
    assert!(g.abs() < 1e-12, "g(beta) = {g:e} at beta = {beta}");
}

/// No root means no root. Clamping to 0 or 1 would turn a contradiction between the stability test
/// and the flash into a silently single-phase answer.
#[test]
fn comp_rr_reports_no_root_rather_than_clamping() {
    // Every K above one: the mixture is all vapour and g never changes sign.
    assert!(matches!(
        solve_rachford_rice(&[3.0, 2.0], &[0.5, 0.5]),
        Err(FlashError::RachfordRiceNoRoot { .. })
    ));
    // Every K below one: all liquid.
    assert!(matches!(
        solve_rachford_rice(&[0.3, 0.2], &[0.5, 0.5]),
        Err(FlashError::RachfordRiceNoRoot { .. })
    ));
}

/// An absent component contributes nothing to `g`, so a ternary with a zero component must give
/// exactly the binary's answer — not approximately, exactly, because the terms are skipped.
#[test]
fn comp_rr_ignores_absent_components_exactly() {
    let ternary = solve_rachford_rice(&[1.0, 3.0, 0.2], &[0.0, 0.6, 0.4]).unwrap();
    let binary = solve_rachford_rice(&[3.0, 0.2], &[0.6, 0.4]).unwrap();
    assert_eq!(ternary, binary);
}

// ---------------------------------------------------------------------------------------------
// Flash against the fixture
// ---------------------------------------------------------------------------------------------

/// The headline comparison: phase state, vapour fraction and both phase compositions against
/// OPM's, on every flashed state.
#[test]
fn comp_flash_matches_the_fixture_on_every_state() {
    let f = fixture::load();
    let mut worst_beta = (0.0f64, String::new());
    let mut worst_x = (0.0f64, String::new());
    let mut worst_residual = (0.0f64, String::new());
    let mut two_phase = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            let result = flash(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap_or_else(|e| panic!("{}: flash failed: {e}", state.id));

            match state.present_phase.as_str() {
                "two_phase" => {
                    assert_eq!(result.phase_state, PhaseState::TwoPhase, "{}", state.id);
                    two_phase += 1;

                    // OPM reports L; this reports beta. The conversion is the one the plan warns
                    // about, so it is done through the named accessor.
                    let d = (result.l_liquid() - state.l_liquid.unwrap()).abs();
                    if d > worst_beta.0 {
                        worst_beta = (d, state.id.clone());
                    }

                    for i in 0..spec.component_count() {
                        for (ours, theirs, tag) in [
                            (result.x[i], state.x_liquid.as_ref().unwrap()[i], "x"),
                            (result.y[i], state.y_vapour.as_ref().unwrap()[i], "y"),
                        ] {
                            let d = (ours - theirs).abs();
                            if d > worst_x.0 {
                                worst_x = (d, format!("{}/{tag}{i}", state.id));
                            }
                        }
                    }

                    if result.equilibrium_residual > worst_residual.0 {
                        worst_residual = (result.equilibrium_residual, state.id.clone());
                    }
                }
                "liquid" => {
                    assert_eq!(result.phase_state, PhaseState::SingleLiquid, "{}", state.id)
                }
                "vapour" => {
                    assert_eq!(result.phase_state, PhaseState::SingleVapour, "{}", state.id)
                }
                other => panic!("{}: unexpected present_phase {other}", state.id),
            }
        }
    }

    assert_eq!(two_phase, 29);
    assert!(
        worst_beta.0 < 1e-8,
        "vapour fraction disagrees: worst absolute error {:e} at {}",
        worst_beta.0,
        worst_beta.1
    );
    assert!(
        worst_x.0 < 1e-8,
        "phase compositions disagree: worst absolute error {:e} at {}",
        worst_x.0,
        worst_x.1
    );
    assert!(
        worst_residual.0 < 1e-11,
        "our own equilibrium residual is too loose: {:e} at {}",
        worst_residual.0,
        worst_residual.1
    );
}

/// Every two-phase result must satisfy the three invariants that *define* a flash, independently
/// of what OPM said: both phase compositions normalized, `z` recovered from `beta`, `x` and `y`,
/// and equal component fugacities.
#[test]
fn comp_flash_satisfies_its_defining_invariants() {
    let f = fixture::load();
    let mut worst_norm = 0.0f64;
    let mut worst_recon = (0.0f64, String::new());
    let mut worst_fug = (0.0f64, String::new());

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            let r = flash(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap();
            if r.phase_state != PhaseState::TwoPhase {
                continue;
            }

            worst_norm = worst_norm
                .max((r.x.iter().sum::<f64>() - 1.0).abs())
                .max((r.y.iter().sum::<f64>() - 1.0).abs());

            for i in 0..spec.component_count() {
                let reconstructed = (1.0 - r.beta) * r.x[i] + r.beta * r.y[i];
                let d = (reconstructed - state.z[i]).abs();
                if d > worst_recon.0 {
                    worst_recon = (d, format!("{}/{i}", state.id));
                }

                if state.z[i] == 0.0 {
                    continue;
                }
                let liquid = r.liquid.as_ref().unwrap();
                let vapour = r.vapour.as_ref().unwrap();
                let ln_ratio = r.x[i].ln() + liquid.ln_phi[i] - r.y[i].ln() - vapour.ln_phi[i];
                if ln_ratio.abs() > worst_fug.0 {
                    worst_fug = (ln_ratio.abs(), format!("{}/{i}", state.id));
                }
            }
        }
    }

    // The plan's admission targets, all three.
    assert!(worst_norm <= 1e-12, "normalization: {worst_norm:e}");
    assert!(
        worst_recon.0 <= 1e-10,
        "z reconstruction: {:e} at {}",
        worst_recon.0,
        worst_recon.1
    );
    assert!(
        worst_fug.0 <= 1e-8,
        "log-fugacity ratio: {:e} at {}",
        worst_fug.0,
        worst_fug.1
    );
}

/// The converged state must not depend on the guess it started from. C10 will warm-start every
/// cell from the previous timestep, so a flash whose answer drifted with its seed would make the
/// trajectory a function of iteration history.
#[test]
fn comp_flash_converges_to_the_same_state_from_warm_and_cold_starts() {
    let f = fixture::load();
    let mut worst = (0.0f64, String::new());

    for system in &f.systems {
        let spec = spec_for(system);
        let n = spec.component_count();
        for state in &system.states {
            let cold = flash(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap();

            let wilson = wilson_k(&spec, state.pressure_pa, state.temperature_k);
            let seeds: [Vec<f64>; 3] = [
                cold.k.clone(),
                wilson.iter().map(|k| k * 5.0).collect(),
                wilson.iter().map(|k| k / 5.0).collect(),
            ];
            for (which, seed) in seeds.iter().enumerate() {
                let warm = flash(
                    &spec,
                    state.pressure_pa,
                    state.temperature_k,
                    &state.z,
                    Some(seed),
                )
                .unwrap_or_else(|e| panic!("{} seed {which}: {e}", state.id));
                assert_eq!(
                    warm.phase_state, cold.phase_state,
                    "{} seed {which}: phase state changed",
                    state.id
                );
                let d = (warm.beta - cold.beta).abs();
                if d > worst.0 {
                    worst = (d, format!("{} seed {which}", state.id));
                }
                for i in 0..n {
                    worst.0 = worst.0.max((warm.x[i] - cold.x[i]).abs());
                    worst.0 = worst.0.max((warm.y[i] - cold.y[i]).abs());
                }
            }
        }
    }
    assert!(
        worst.0 < 1e-9,
        "the converged state depends on the starting guess: worst drift {:e} at {}",
        worst.0,
        worst.1
    );
}

/// A stable feed is a success. It must report an exact phase amount, the feed's own composition,
/// and properties for the phase that exists and none for the one that does not.
#[test]
fn comp_flash_single_phase_is_a_success_with_an_exact_phase_amount() {
    let f = fixture::load();
    let mut liquid_states = 0;
    let mut vapour_states = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            if state.is_two_phase() {
                continue;
            }
            let r = flash(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap();

            assert_eq!(
                r.x, state.z,
                "{}: liquid composition must be the feed",
                state.id
            );
            assert_eq!(
                r.y, state.z,
                "{}: vapour composition must be the feed",
                state.id
            );
            assert_eq!(r.equilibrium_residual, 0.0, "{}", state.id);
            assert_eq!(r.iterations, 0, "{}", state.id);

            match r.phase_state {
                PhaseState::SingleLiquid => {
                    liquid_states += 1;
                    assert_eq!(r.beta, 0.0, "{}: beta must be exactly zero", state.id);
                    assert!(r.liquid.is_some() && r.vapour.is_none(), "{}", state.id);
                    assert_eq!(r.liquid_saturation(), 1.0, "{}", state.id);
                }
                PhaseState::SingleVapour => {
                    vapour_states += 1;
                    assert_eq!(r.beta, 1.0, "{}: beta must be exactly one", state.id);
                    assert!(r.vapour.is_some() && r.liquid.is_none(), "{}", state.id);
                    assert_eq!(r.vapour_saturation(), 1.0, "{}", state.id);
                }
                PhaseState::TwoPhase => panic!("{}: expected a single phase", state.id),
            }
        }
    }
    assert_eq!(liquid_states, 15);
    assert_eq!(vapour_states, 3);
}

/// Saturation is a volume fraction and `beta` is a mole fraction. They are equal only when both
/// phases happen to have the same molar density, which essentially never holds — so a state where
/// they coincide would mean the saturation was computed from the wrong quantity.
#[test]
fn comp_flash_vapour_saturation_is_not_the_vapour_mole_fraction() {
    let f = fixture::load();
    let mut worst_gap = 0.0f64;
    let mut checked = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            let r = flash(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap();
            if r.phase_state != PhaseState::TwoPhase {
                continue;
            }
            checked += 1;

            let sv = r.vapour_saturation();
            assert!(sv > 0.0 && sv < 1.0, "{}: S_V = {sv}", state.id);
            assert!(
                (r.liquid_saturation() + sv - 1.0).abs() < 1e-14,
                "{}: saturations do not sum to one",
                state.id
            );

            // The vapour is the less dense phase, so it always occupies more volume than its mole
            // fraction would suggest.
            assert!(
                sv > r.beta,
                "{}: S_V = {sv} is not above beta = {}; the vapour is the lighter phase",
                state.id,
                r.beta
            );
            worst_gap = worst_gap.max(sv - r.beta);

            // And the mixture molar volume must reproduce the phase split.
            let v = r.mixture_molar_volume();
            let expected = (1.0 - r.beta) / r.liquid.as_ref().unwrap().molar_density
                + r.beta / r.vapour.as_ref().unwrap().molar_density;
            assert!((v - expected).abs() / expected < 1e-15, "{}", state.id);
        }
    }
    assert_eq!(checked, 29);
    assert!(
        worst_gap > 0.1,
        "S_V and beta never differ materially; this test is not exercising its case"
    );
}

/// An exactly zero component must stay exactly zero in both phases. Nothing in the flash may
/// create material, and a component that is absent in the feed cannot appear in a product phase.
#[test]
fn comp_flash_preserves_an_exactly_zero_component() {
    let ternary = pinned::ternary().unwrap();
    let binary = pinned::binary().unwrap();
    let t = 423.15;

    for p in [2.0e6, 5.0e6, 1.5e7, 2.0e7] {
        let r = flash(&ternary, p, t, &[0.0, 0.6, 0.4], None)
            .unwrap_or_else(|e| panic!("p = {p}: {e}"));
        assert_eq!(r.x[0], 0.0, "p = {p}: CO2 appeared in the liquid");
        assert_eq!(r.y[0], 0.0, "p = {p}: CO2 appeared in the vapour");

        // And the answer must be the binary's.
        let b = flash(&binary, p, t, &[0.6, 0.4], None).unwrap();
        assert_eq!(r.phase_state, b.phase_state, "p = {p}");
        assert!(
            (r.beta - b.beta).abs() < 1e-9,
            "p = {p}: zero-CO2 ternary beta {} vs binary {}",
            r.beta,
            b.beta
        );
        for i in 0..2 {
            assert!((r.x[i + 1] - b.x[i]).abs() < 1e-9, "p = {p}, x[{i}]");
            assert!((r.y[i + 1] - b.y[i]).abs() < 1e-9, "p = {p}, y[{i}]");
        }
    }
}

/// A trace component must be distributed, not dropped. At 1e-6 of CO2 the mixture is physically
/// the binary, but the CO2 must still appear in both phases with a sensible K.
#[test]
fn comp_flash_distributes_a_trace_component() {
    let spec = pinned::ternary().unwrap();
    let r = flash(&spec, 1.5e7, 423.15, &[1e-6, 0.6, 0.399999], None).unwrap();
    assert_eq!(r.phase_state, PhaseState::TwoPhase);

    assert!(
        r.x[0] > 0.0 && r.y[0] > 0.0,
        "the trace component was dropped"
    );
    // CO2 is more volatile than n-decane at these conditions, so it must favour the vapour.
    let k_co2 = r.y[0] / r.x[0];
    assert!(
        k_co2 > 1.0,
        "trace CO2 should favour the vapour: K = {k_co2}"
    );
    // And it must be conserved.
    let recovered = (1.0 - r.beta) * r.x[0] + r.beta * r.y[0];
    assert!(
        (recovered - 1e-6).abs() < 1e-14,
        "trace CO2 not conserved: {recovered}"
    );
}

#[test]
fn comp_flash_rejects_an_empty_or_mismatched_feed() {
    let spec = pinned::ternary().unwrap();
    assert!(matches!(
        flash(&spec, 1.5e7, 423.15, &[0.5, 0.5], None),
        Err(FlashError::InvalidFeed { .. })
    ));
    assert!(matches!(
        flash(&spec, 1.5e7, 423.15, &[0.0, 0.0, 0.0], None),
        Err(FlashError::InvalidFeed { .. })
    ));
}

/// Iteration counts, recorded so a later performance change has a baseline to move against rather
/// than a vague impression.
#[test]
fn comp_flash_iteration_counts_are_bounded() {
    let f = fixture::load();
    let mut worst = (0usize, String::new());
    let mut total = 0usize;
    let mut two_phase = 0usize;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            let r = flash(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap();
            if r.phase_state != PhaseState::TwoPhase {
                continue;
            }
            two_phase += 1;
            total += r.iterations;
            if r.iterations > worst.0 {
                worst = (r.iterations, state.id.clone());
            }
        }
    }

    let mean = total as f64 / two_phase as f64;
    assert!(
        worst.0 < super::flash::MAX_SUBSTITUTION_ITERATIONS,
        "{} hit the iteration cap",
        worst.1
    );
    // Successive substitution is linearly convergent, so these are not small numbers and are not
    // meant to be; the point is that they are stable and measured. C14 owns making them smaller.
    assert!(
        mean < 2000.0,
        "mean substitution count {mean:.0} is higher than recorded; worst is {} at {}",
        worst.0,
        worst.1
    );
}

/// Both EOS branches must be used: a flash that evaluated the vapour on the liquid root would give
/// two liquids and still satisfy fugacity equality between them.
#[test]
fn comp_flash_uses_the_correct_branch_for_each_phase() {
    let f = fixture::load();
    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            let r = flash(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap();
            if let Some(l) = &r.liquid {
                assert_eq!(l.branch, PhaseBranch::Liquid, "{}", state.id);
            }
            if let Some(v) = &r.vapour {
                assert_eq!(v.branch, PhaseBranch::Vapour, "{}", state.id);
            }
            if r.phase_state == PhaseState::TwoPhase {
                let l = r.liquid.as_ref().unwrap();
                let v = r.vapour.as_ref().unwrap();
                assert!(
                    l.molar_density > v.molar_density,
                    "{}: the liquid is not the denser phase",
                    state.id
                );
            }
        }
    }
}

/// Single-phase labelling uses Li's pseudo-critical temperature, and the obvious alternatives get
/// it wrong. At 250 bar the C1/C10 binary has `Z = 0.975` and a mass density of 484 kg/m³: a
/// compressibility-factor heuristic calls that a vapour, OPM calls it a liquid, and OPM is right —
/// it is a dense supercritical liquid.
#[test]
fn comp_flash_single_phase_label_follows_li_not_the_compressibility_factor() {
    let spec = pinned::binary().unwrap();
    let z = [0.6, 0.4];
    let t = 423.15;

    let tc_est = li_pseudo_critical_temperature(&spec, &z);
    // Critical-volume weighting puts the estimate far above the mole-fraction-weighted mean,
    // because n-decane's critical volume is six times methane's.
    assert!(tc_est > 500.0 && tc_est < 620.0, "Tc_est = {tc_est}");
    assert!(t < tc_est, "the mixture must label as liquid at 423 K");

    let r = flash(&spec, 2.5e7, t, &z, None).unwrap();
    assert_eq!(r.phase_state, PhaseState::SingleLiquid);
    let z_factor = r.liquid.as_ref().unwrap().z_factor;
    assert!(
        z_factor > 0.9,
        "this test is pointless unless Z looks vapour-like: {z_factor}"
    );

    // Above the pseudo-critical temperature the same composition labels as vapour.
    let hot = flash(&spec, 2.5e7, tc_est + 50.0, &z, None).unwrap();
    assert_eq!(hot.phase_state, PhaseState::SingleVapour);
}

/// The estimate must reduce to a pure component's own critical temperature, and move
/// monotonically between the two as the composition sweeps.
#[test]
fn comp_flash_li_estimate_reduces_to_pure_component_critical_temperatures() {
    let spec = pinned::binary().unwrap();
    assert!((li_pseudo_critical_temperature(&spec, &[1.0, 0.0]) - 190.6).abs() < 1e-12);
    assert!((li_pseudo_critical_temperature(&spec, &[0.0, 1.0]) - 617.7).abs() < 1e-12);

    let mut previous = li_pseudo_critical_temperature(&spec, &[1.0, 0.0]);
    for step in 1..=20 {
        let f = step as f64 / 20.0;
        let tc = li_pseudo_critical_temperature(&spec, &[1.0 - f, f]);
        assert!(
            tc > previous,
            "Tc_est is not monotonic in composition at f = {f}"
        );
        previous = tc;
    }
}
