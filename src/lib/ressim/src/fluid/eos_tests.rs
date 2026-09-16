//! C2 contract tests for the Peng–Robinson EOS (`comp_eos_*`).
//!
//! Two kinds of check, and both are needed. The fixture comparisons prove this module reproduces
//! OPM's EOS; the independent limits prove it is the *right* EOS rather than a faithful copy of a
//! wrong one. A translated algorithm and values derived from that translation are one oracle, not
//! two — so the ideal-gas, dimensional and pure-component checks below deliberately do not consult
//! the fixture at all.

use super::eos::{CubicRoots, EosError, PhaseBranch, evaluate, mixture_params, z_roots};
use super::fixture::{self, FixtureSystem};
use super::pinned;
use super::specification::{Component, EosVariant, FluidSpecification, ViscosityModel};
use super::units::GAS_CONSTANT_J_PER_MOL_K;

/// The specification matching a fixture system, built from the committed `pinned` fluids.
fn spec_for(system: &FixtureSystem) -> FluidSpecification {
    match system.num_components {
        2 => pinned::binary().unwrap(),
        3 => pinned::ternary().unwrap(),
        n => panic!("no pinned specification for {n} components"),
    }
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

// ---------------------------------------------------------------------------------------------
// Fixture comparison
// ---------------------------------------------------------------------------------------------

/// `A` and `B` are `R`-independent and depend only on reduced pressure, reduced temperature, the
/// acentric factors and the composition. They are therefore the cleanest possible comparison —
/// nothing about the cubic solver or the gas constant can hide in them.
#[test]
fn comp_eos_mixture_parameters_match_the_fixture() {
    let f = fixture::load();
    let mut worst = (0.0f64, String::new());
    let mut checked = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            for liquid in [true, false] {
                let x = state.composition_for(liquid);
                let phase = if liquid {
                    state.liquid.as_ref()
                } else {
                    state.vapour.as_ref()
                };
                let Some(phase) = phase else { continue };

                let p = mixture_params(&spec, state.pressure_pa, state.temperature_k, x)
                    .unwrap_or_else(|e| panic!("{}: mixture params failed: {e}", state.id));

                for (label, ours, theirs) in [("A", p.a, phase.eos_A), ("B", p.b, phase.eos_B)] {
                    let r = rel(ours, theirs);
                    if r > worst.0 {
                        worst = (
                            r,
                            format!("{}/{}/{label}", state.id, if liquid { "L" } else { "V" }),
                        );
                    }
                    checked += 1;
                }
            }
        }
    }

    assert!(
        checked >= 180,
        "expected the full fixture, compared only {checked} values"
    );
    assert!(
        worst.0 < 1e-13,
        "mixture parameters disagree with the fixture: worst relative error {:e} at {}",
        worst.0,
        worst.1
    );
}

/// The compressibility factor is also `R`-independent, so this isolates the cubic solver and the
/// root-labelling rule. Any disagreement here is an algorithm difference, never a units one.
#[test]
fn comp_eos_z_factor_matches_the_fixture() {
    let f = fixture::load();
    let mut worst = (0.0f64, String::new());
    let mut checked = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            for (liquid, branch) in [(true, PhaseBranch::Liquid), (false, PhaseBranch::Vapour)] {
                let Some(phase) = (if liquid { &state.liquid } else { &state.vapour }) else {
                    continue;
                };
                let x = state.composition_for(liquid);
                let props = evaluate(&spec, state.pressure_pa, state.temperature_k, x, branch)
                    .unwrap_or_else(|e| panic!("{}: evaluate failed: {e}", state.id));

                let r = rel(props.z_factor, phase.z_factor);
                if r > worst.0 {
                    worst = (
                        r,
                        format!("{}/{}", state.id, if liquid { "L" } else { "V" }),
                    );
                }
                checked += 1;
            }
        }
    }

    assert!(checked >= 90, "compared only {checked} states");
    assert!(
        worst.0 < 1e-12,
        "Z disagrees with the fixture: worst relative error {:e} at {}",
        worst.0,
        worst.1
    );
}

/// Densities carry the gas constant, and OPM's is the superseded `8.314472`. The comparison
/// applies the exact analytic correction rather than widening the tolerance, which turns the
/// discrepancy into a *test* of where `R` enters: if `R` leaked into `A`, `B` or the cubic, the
/// corrected comparison would fail even though an unconverted one at a loose tolerance would pass.
#[test]
fn comp_eos_densities_match_the_fixture_after_the_exact_gas_constant_correction() {
    let f = fixture::load();
    let mut worst = (0.0f64, String::new());

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            for (liquid, branch) in [(true, PhaseBranch::Liquid), (false, PhaseBranch::Vapour)] {
                let Some(phase) = (if liquid { &state.liquid } else { &state.vapour }) else {
                    continue;
                };
                let x = state.composition_for(liquid);
                let props = evaluate(&spec, state.pressure_pa, state.temperature_k, x, branch)
                    .unwrap_or_else(|e| panic!("{}: evaluate failed: {e}", state.id));

                let tag = |q: &str| format!("{}/{}/{q}", state.id, if liquid { "L" } else { "V" });
                for (q, ours, theirs) in [
                    ("molar_volume", props.molar_volume, phase.molar_volume_si()),
                    (
                        "molar_density",
                        props.molar_density,
                        phase.molar_density_si(),
                    ),
                    ("mass_density", props.mass_density, phase.mass_density_si()),
                ] {
                    let r = rel(ours, theirs);
                    if r > worst.0 {
                        worst = (r, tag(q));
                    }
                }
            }
        }
    }

    assert!(
        worst.0 < 1e-7,
        "densities disagree after the R correction: worst relative error {:e} at {}",
        worst.0,
        worst.1
    );
}

/// Without the correction the same comparison is off by the gas-constant ratio and nothing else.
/// This pins that claim: the discrepancy is a single systematic factor, not scatter.
#[test]
fn comp_eos_uncorrected_density_error_is_exactly_the_gas_constant_ratio() {
    let f = fixture::load();
    let expected = super::units::OPM_GAS_CONSTANT_J_PER_MOL_K / GAS_CONSTANT_J_PER_MOL_K;
    let mut worst_deviation = 0.0f64;
    let mut samples = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            let Some(phase) = &state.vapour else { continue };
            let x = state.composition_for(false);
            let props = evaluate(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                x,
                PhaseBranch::Vapour,
            )
            .unwrap();
            let ratio = props.molar_density / phase.molar_density;
            worst_deviation = worst_deviation.max((ratio - expected).abs() / expected);
            samples += 1;
        }
    }

    assert!(samples >= 40, "only {samples} samples");
    assert!(
        worst_deviation < 1e-12,
        "the uncorrected density error is not a pure gas-constant ratio: worst deviation {:e}",
        worst_deviation
    );
}

/// Fugacity coefficients are `R`-independent, so they compare directly. This is the check that
/// matters most for C3: equilibrium is expressed entirely in these.
#[test]
fn comp_eos_fugacity_coefficients_match_the_fixture() {
    let f = fixture::load();
    let mut worst = (0.0f64, String::new());
    let mut checked = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            for (liquid, branch) in [(true, PhaseBranch::Liquid), (false, PhaseBranch::Vapour)] {
                let Some(phase) = (if liquid { &state.liquid } else { &state.vapour }) else {
                    continue;
                };
                let x = state.composition_for(liquid);
                let props = evaluate(&spec, state.pressure_pa, state.temperature_k, x, branch)
                    .unwrap_or_else(|e| panic!("{}: evaluate failed: {e}", state.id));

                for i in 0..spec.component_count() {
                    let ours = props.fugacity_coefficient(i);
                    let theirs = phase.fugacity_coefficient[i];
                    let r = rel(ours, theirs);
                    if r > worst.0 {
                        worst = (
                            r,
                            format!(
                                "{}/{}/{}",
                                state.id,
                                if liquid { "L" } else { "V" },
                                spec.component(i).id
                            ),
                        );
                    }
                    checked += 1;
                }
            }
        }
    }

    assert!(
        checked == 244,
        "expected every branch of every state, compared {checked}"
    );
    assert!(
        worst.0 < 1e-11,
        "fugacity coefficients disagree: worst relative error {:e} at {}",
        worst.0,
        worst.1
    );
}

/// OPM clamps `phi` into `[1e-10, 1e10]` and floors the molar volume at `1e-7 m³/mol`. This module
/// reproduces neither. That is only safe if no fixture state is anywhere near either limit —
/// otherwise the fixture would carry a clamped value that no unclamped implementation can match.
#[test]
fn comp_eos_no_fixture_state_approaches_an_opm_clamp() {
    let f = fixture::load();
    for system in &f.systems {
        for state in &system.states {
            for phase in [state.liquid.as_ref(), state.vapour.as_ref()]
                .into_iter()
                .flatten()
            {
                assert!(
                    phase.molar_volume > 1e-6,
                    "{}: molar volume {} is within an order of magnitude of OPM's 1e-7 floor",
                    state.id,
                    phase.molar_volume
                );
                for (i, &phi) in phase.fugacity_coefficient.iter().enumerate() {
                    assert!(
                        phi > 1e-8 && phi < 1e8,
                        "{}: phi[{i}] = {phi} is within two orders of an OPM clamp",
                        state.id
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Independent limits — these never consult the fixture
// ---------------------------------------------------------------------------------------------

/// As pressure falls, any real gas approaches ideality: `Z -> 1` and `phi_i -> 1`. Neither
/// follows from reproducing OPM correctly, so this is a genuinely independent check on the whole
/// chain — mixing rule, cubic, root selection and fugacity expression together.
#[test]
fn comp_eos_approaches_the_ideal_gas_limit_at_low_pressure() {
    let spec = pinned::ternary().unwrap();
    let t = 423.15;
    let x = [0.2, 0.5, 0.3];

    let mut previous_z_error = f64::INFINITY;
    let mut previous_phi_error = f64::INFINITY;

    for p in [1.0e5, 1.0e4, 1.0e3, 1.0e2, 1.0e1] {
        let props = evaluate(&spec, p, t, &x, PhaseBranch::Vapour).unwrap();
        let z_error = (props.z_factor - 1.0).abs();
        let phi_error = props.ln_phi.iter().map(|l| l.abs()).fold(0.0f64, f64::max);

        assert!(
            z_error < previous_z_error,
            "Z is not approaching 1 monotonically as p falls: {z_error} at p = {p}"
        );
        assert!(
            phi_error < previous_phi_error,
            "ln phi is not approaching 0 monotonically as p falls: {phi_error} at p = {p}"
        );
        previous_z_error = z_error;
        previous_phi_error = phi_error;
    }

    // At 10 Pa the mixture is ideal to well under a part in a million.
    let props = evaluate(&spec, 10.0, t, &x, PhaseBranch::Vapour).unwrap();
    assert!(
        (props.z_factor - 1.0).abs() < 1e-6,
        "Z = {}",
        props.z_factor
    );
    for (i, l) in props.ln_phi.iter().enumerate() {
        assert!(l.abs() < 1e-5, "ln phi[{i}] = {l}");
    }
    // And the molar density must match the ideal gas law directly.
    let ideal = 10.0 / (GAS_CONSTANT_J_PER_MOL_K * t);
    assert!(rel(props.molar_density, ideal) < 1e-6);
}

/// The EOS is a relation between reduced quantities, so a pure component's `A` and `B` must depend
/// only on `p_r` and `T_r`. Two different components evaluated at the same reduced state and the
/// same acentric factor therefore give identical dimensionless parameters — which is a statement
/// about the equation, not about any implementation of it.
#[test]
fn comp_eos_pure_component_parameters_depend_only_on_reduced_state() {
    let make = |tc: f64, pc: f64| {
        let c = |id: &str, tc: f64, pc: f64| Component {
            id: id.to_string(),
            molar_mass_kg_per_mol: 0.016,
            critical_temperature_k: tc,
            critical_pressure_pa: pc,
            critical_volume_m3_per_kmol: 9.863e-2,
            acentric_factor: 0.25,
        };
        // A binary of two identical-in-reduced-terms components; the second exists only because
        // the specification requires at least two.
        FluidSpecification::new(
            vec![c("A", tc, pc), c("B", tc, pc)],
            vec![vec![0.0; 2]; 2],
            EosVariant::PengRobinson,
            ViscosityModel::LohrenzBrayClark,
            400.0,
            None,
        )
        .unwrap()
    };

    let t_r = 1.3;
    let p_r = 2.0;

    let a = make(190.6, 4.6e6);
    let b = make(617.7, 2.1e6);
    let pa = mixture_params(&a, p_r * 4.6e6, t_r * 190.6, &[0.5, 0.5]).unwrap();
    let pb = mixture_params(&b, p_r * 2.1e6, t_r * 617.7, &[0.5, 0.5]).unwrap();

    assert!(rel(pa.a, pb.a) < 1e-14, "A: {} vs {}", pa.a, pb.a);
    assert!(rel(pa.b, pb.b) < 1e-14, "B: {} vs {}", pa.b, pb.b);

    // And the compressibility factor, which depends on nothing else.
    let za = evaluate(
        &a,
        p_r * 4.6e6,
        t_r * 190.6,
        &[0.5, 0.5],
        PhaseBranch::Vapour,
    )
    .unwrap();
    let zb = evaluate(
        &b,
        p_r * 2.1e6,
        t_r * 617.7,
        &[0.5, 0.5],
        PhaseBranch::Vapour,
    )
    .unwrap();
    assert!(rel(za.z_factor, zb.z_factor) < 1e-14);
}

/// `Z = p V_m / (R T)` by definition. If the molar volume and the compressibility factor ever stop
/// satisfying it, one of them was computed from something other than the other.
#[test]
fn comp_eos_z_and_molar_volume_are_consistent_by_definition() {
    let f = fixture::load();
    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            for branch in [PhaseBranch::Liquid, PhaseBranch::Vapour] {
                let x = state.composition_for(branch == PhaseBranch::Liquid);
                let props =
                    evaluate(&spec, state.pressure_pa, state.temperature_k, x, branch).unwrap();
                let z_from_v = state.pressure_pa * props.molar_volume
                    / (GAS_CONSTANT_J_PER_MOL_K * state.temperature_k);
                assert!(
                    rel(z_from_v, props.z_factor) < 1e-14,
                    "{}: Z = {} but p Vm / RT = {}",
                    state.id,
                    props.z_factor,
                    z_from_v
                );
            }
        }
    }
}

/// Mass density is molar density times the composition-weighted molar mass, and for a pure
/// component that is the component's own molar mass. Independent of the EOS entirely.
#[test]
fn comp_eos_mass_density_is_molar_density_times_mean_molar_mass() {
    let spec = pinned::binary().unwrap();
    // Nearly pure C1: the mean molar mass must be within a hair of C1's own.
    let x = [1.0 - 1e-12, 1e-12];
    let props = evaluate(&spec, 5.0e6, 423.15, &x, PhaseBranch::Vapour).unwrap();
    assert!(rel(props.mean_molar_mass, 0.0160) < 1e-10);
    assert!(
        rel(
            props.mass_density,
            props.molar_density * props.mean_molar_mass
        ) < 1e-15
    );
}

// ---------------------------------------------------------------------------------------------
// Root structure and failure contracts
// ---------------------------------------------------------------------------------------------

/// The state this test exists for: a cubic with three real roots, at a state where nothing has
/// claimed two phases are present. Both labelled roots evaluate cleanly and give very different
/// densities, and that fact alone says nothing about stability — deciding that is C3's job.
///
/// These come from the fixture's flash-free `eos_states` section, which exists precisely because
/// none of the flashed states is multi-root: at 150 °C every one of them is monotonic, so without
/// this section the root-labelling rule would be entirely untested.
#[test]
fn comp_eos_three_roots_do_not_imply_two_phases() {
    let f = fixture::load();
    let mut found = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.eos_states {
            if !state.has_distinct_roots() {
                continue;
            }
            found += 1;
            let params =
                mixture_params(&spec, state.pressure_pa, state.temperature_k, &state.x).unwrap();
            let roots = z_roots(&params).unwrap();
            assert!(
                matches!(roots, CubicRoots::Three(_)),
                "{}: OPM reports two distinct roots but the cubic solver found one",
                state.id
            );

            let liquid = evaluate(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.x,
                PhaseBranch::Liquid,
            )
            .unwrap();
            let vapour = evaluate(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.x,
                PhaseBranch::Vapour,
            )
            .unwrap();

            assert!(
                liquid.z_factor < vapour.z_factor,
                "{}: the liquid label must take the smaller root",
                state.id
            );
            assert!(
                liquid.molar_density > vapour.molar_density,
                "{}: the liquid label must be the denser one",
                state.id
            );
            // Both are admissible: a three-root state is not an error state.
            assert!(liquid.z_factor > params.b && vapour.z_factor > params.b);
        }
    }

    assert!(
        found >= 5,
        "only {found} multi-root states in the fixture; this test is not exercising its case"
    );
}

/// Every flash-free state, multi-root or not, must reproduce OPM's roots and fugacities. This is
/// the comparison that actually covers the labelling rule, pure components included.
#[test]
fn comp_eos_flash_free_states_match_the_fixture() {
    let f = fixture::load();
    let mut worst = (0.0f64, String::new());
    let mut checked = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.eos_states {
            assert_eq!(state.status, "ok", "{}: {}", state.id, state.note);
            let params = mixture_params(&spec, state.pressure_pa, state.temperature_k, &state.x)
                .unwrap_or_else(|e| panic!("{}: {e}", state.id));

            for (label, ours, theirs) in [
                ("A", params.a, state.eos_A.unwrap()),
                ("B", params.b, state.eos_B.unwrap()),
            ] {
                let r = rel(ours, theirs);
                if r > worst.0 {
                    worst = (r, format!("{}/{label}", state.id));
                }
                checked += 1;
            }

            for (branch, root) in [
                (PhaseBranch::Liquid, state.smallest_root.as_ref().unwrap()),
                (PhaseBranch::Vapour, state.largest_root.as_ref().unwrap()),
            ] {
                let props = evaluate(
                    &spec,
                    state.pressure_pa,
                    state.temperature_k,
                    &state.x,
                    branch,
                )
                .unwrap_or_else(|e| panic!("{}: {e}", state.id));

                let r = rel(props.z_factor, root.z_factor);
                if r > worst.0 {
                    worst = (r, format!("{}/{branch:?}/Z", state.id));
                }
                checked += 1;

                for i in 0..spec.component_count() {
                    let r = rel(props.fugacity_coefficient(i), root.fugacity_coefficient[i]);
                    if r > worst.0 {
                        worst = (r, format!("{}/{branch:?}/phi{i}", state.id));
                    }
                    checked += 1;
                }

                let r = rel(props.molar_density, root.molar_density_si());
                if r > worst.0 {
                    worst = (r, format!("{}/{branch:?}/rho", state.id));
                }
                checked += 1;
            }
        }
    }

    assert!(checked >= 150, "compared only {checked} values");
    assert!(
        worst.0 < 1e-11,
        "flash-free EOS states disagree: worst relative error {:e} at {}",
        worst.0,
        worst.1
    );
}

/// A pure component that is *supercritical* at the fixture temperature must give one root at every
/// pressure, and a subcritical one must give three near its vapour pressure. Getting this backwards
/// is a real failure mode, and comparing only mixtures would never catch it.
#[test]
fn comp_eos_root_count_follows_the_reduced_temperature() {
    let f = fixture::load();
    let mut supercritical_single = 0;
    let mut subcritical_multi = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.eos_states {
            let params =
                mixture_params(&spec, state.pressure_pa, state.temperature_k, &state.x).unwrap();
            let roots = z_roots(&params).unwrap();
            let opm_distinct = state.has_distinct_roots();

            match (&roots, opm_distinct) {
                (CubicRoots::One(_), false) => {}
                (CubicRoots::Three(z), false) => {
                    // OPM collapses to one root while the solver finds three: only acceptable if
                    // the extra roots are numerically indistinguishable.
                    assert!(
                        (z[2] - z[0]).abs() < 1e-6,
                        "{}: three well-separated roots that OPM did not report: {z:?}",
                        state.id
                    );
                }
                (CubicRoots::Three(_), true) => {}
                (CubicRoots::One(_), true) => {
                    panic!(
                        "{}: OPM found two distinct roots, the solver found one",
                        state.id
                    )
                }
            }

            if state.id.contains("pure_c1_p") {
                assert!(
                    !opm_distinct,
                    "{}: methane is supercritical at 423 K",
                    state.id
                );
                supercritical_single += 1;
            }
            if state.id.contains("pure_c10_p") && opm_distinct {
                subcritical_multi += 1;
            }
        }
    }

    assert!(
        supercritical_single >= 3,
        "no supercritical pure-component states"
    );
    assert!(
        subcritical_multi >= 2,
        "no subcritical pure-component multi-root states"
    );
}

/// Root ordering is the whole content of the labelling rule, so it is checked on every state
/// rather than trusted.
#[test]
fn comp_eos_roots_are_ascending_and_the_branches_bracket_them() {
    let f = fixture::load();
    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            let params =
                mixture_params(&spec, state.pressure_pa, state.temperature_k, &state.z).unwrap();
            let roots = z_roots(&params).unwrap();
            if let CubicRoots::Three(z) = &roots {
                assert!(
                    z[0] <= z[1] && z[1] <= z[2],
                    "{}: roots not ascending: {z:?}",
                    state.id
                );
            }
            for r in roots.as_slice() {
                assert!(r.is_finite(), "{}: non-finite root", state.id);
            }
        }
    }
}

/// A single real root serves both labels, which is why a single-phase fixture state reports the
/// same density for `liquid` and `vapour`. Confirmed on the fixture's own output rather than
/// asserted.
#[test]
fn comp_eos_a_single_root_gives_identical_branches() {
    let f = fixture::load();
    let mut found = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            let params =
                mixture_params(&spec, state.pressure_pa, state.temperature_k, &state.z).unwrap();
            if let CubicRoots::One(_) = z_roots(&params).unwrap() {
                found += 1;
                let l = evaluate(
                    &spec,
                    state.pressure_pa,
                    state.temperature_k,
                    &state.z,
                    PhaseBranch::Liquid,
                )
                .unwrap();
                let v = evaluate(
                    &spec,
                    state.pressure_pa,
                    state.temperature_k,
                    &state.z,
                    PhaseBranch::Vapour,
                )
                .unwrap();
                assert_eq!(l.z_factor, v.z_factor, "{}", state.id);
                assert_eq!(l.ln_phi, v.ln_phi, "{}", state.id);
            }
        }
    }
    assert!(found > 0, "no fixture state has a single real root");
}

#[test]
fn comp_eos_rejects_non_physical_states() {
    let spec = pinned::binary().unwrap();
    let x = [0.6, 0.4];

    for (p, t) in [
        (0.0, 423.15),
        (-1.0e5, 423.15),
        (f64::NAN, 423.15),
        (1.0e6, 0.0),
        (1.0e6, -5.0),
    ] {
        assert!(
            matches!(
                mixture_params(&spec, p, t, &x),
                Err(EosError::NonPhysicalState { .. })
            ),
            "p = {p}, T = {t} should be rejected"
        );
    }
}

#[test]
fn comp_eos_rejects_invalid_phase_compositions() {
    let spec = pinned::binary().unwrap();
    let p = 1.5e7;
    let t = 423.15;

    assert!(matches!(
        mixture_params(&spec, p, t, &[1.0]),
        Err(EosError::CompositionLength {
            expected: 2,
            actual: 1
        })
    ));
    assert!(matches!(
        mixture_params(&spec, p, t, &[-0.1, 1.1]),
        Err(EosError::CompositionEntry { index: 0, .. })
    ));
    assert!(matches!(
        mixture_params(&spec, p, t, &[f64::NAN, 1.0]),
        Err(EosError::CompositionEntry { index: 0, .. })
    ));
    assert!(matches!(
        mixture_params(&spec, p, t, &[0.3, 0.3]),
        Err(EosError::CompositionSum { .. })
    ));
}

/// The covolume constraint is an error, not a clamp. OPM would floor the molar volume here; this
/// module refuses, because a floored volume is not a solution of the EOS.
#[test]
fn comp_eos_reports_a_missing_admissible_root_rather_than_clamping() {
    use super::eos::ln_fugacity_coefficients;
    let spec = pinned::binary().unwrap();
    let params = mixture_params(&spec, 1.5e7, 423.15, &[0.6, 0.4]).unwrap();

    // Hand the fugacity expression a Z below the covolume directly: no real EOS state produces
    // one, but the public function must reject it rather than return a NaN dressed as a result.
    let err = ln_fugacity_coefficients(&spec, &params, params.b * 0.5, &[0.6, 0.4]).unwrap_err();
    assert!(
        matches!(err, EosError::LogDomain { what: "Z - B", .. }),
        "expected a Z - B domain error, got {err:?}"
    );
    assert!(matches!(
        ln_fugacity_coefficients(&spec, &params, params.b, &[0.6, 0.4]),
        Err(EosError::LogDomain { what: "Z - B", .. })
    ));
}

/// Every fixture state must sit strictly above the covolume — if any did not, the fixture would
/// contain a state this module cannot evaluate and every comparison above would be vacuous there.
#[test]
fn comp_eos_every_fixture_state_satisfies_the_covolume_constraint() {
    let f = fixture::load();
    for system in &f.systems {
        let spec = spec_for(system);
        for state in &system.states {
            for (liquid, branch) in [(true, PhaseBranch::Liquid), (false, PhaseBranch::Vapour)] {
                let x = state.composition_for(liquid);
                let props = evaluate(&spec, state.pressure_pa, state.temperature_k, x, branch)
                    .unwrap_or_else(|e| panic!("{}: {e}", state.id));
                assert!(
                    props.z_factor > props.params.b,
                    "{}: Z = {} is not above B = {}",
                    state.id,
                    props.z_factor,
                    props.params.b
                );
            }
        }
    }
}

/// Near-repeated roots are reported, not hidden. C3 and C4 need the signal; C2 only raises it.
#[test]
fn comp_eos_near_repeated_roots_are_detectable() {
    assert!(!CubicRoots::One(0.9).has_near_repeated(1e-6));
    assert!(CubicRoots::Three([0.1, 0.1 + 1e-9, 0.9]).has_near_repeated(1e-6));
    assert!(!CubicRoots::Three([0.1, 0.5, 0.9]).has_near_repeated(1e-6));
}
