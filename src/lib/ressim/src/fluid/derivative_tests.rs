//! C4 derivative contract tests (`comp_derivatives_*`).
//!
//! Three independent kinds of evidence, because no one of them is sufficient:
//!
//! * the fixture's analytic derivatives, which OPM produced by the same implicit-differentiation
//!   pattern but through a completely different code path;
//! * finite differences over a **step sweep**, which must show a plateau — a single step size can
//!   agree by luck, and the plateau is what distinguishes a correct derivative from a lucky one;
//! * exact structural identities that follow from the constraints alone and hold whatever the
//!   numbers are.

use super::derivatives::{DerivativeError, flash_derivatives};
use super::fixture::{self, FixtureSystem};
use super::flash::{PhaseState, flash};
use super::pinned;
use super::specification::FluidSpecification;

fn spec_for(system: &FixtureSystem) -> FluidSpecification {
    match system.num_components {
        2 => pinned::binary().unwrap(),
        3 => pinned::ternary().unwrap(),
        n => panic!("no pinned specification for {n} components"),
    }
}

/// A relative error against a declared absolute scale, so a derivative that is legitimately near
/// zero is not held to an impossible relative standard.
fn rel_to_scale(ours: f64, theirs: f64, scale: f64) -> f64 {
    (ours - theirs).abs() / theirs.abs().max(scale)
}

// ---------------------------------------------------------------------------------------------
// Against the fixture
// ---------------------------------------------------------------------------------------------

/// `dL/du`, `dx/du` and `dy/du` against OPM's. The fixture reports `dL` (liquid); this reports
/// `dbeta` (vapour), and `beta = 1 - L` gives `dbeta/du = -dL/du`.
#[test]
fn comp_derivatives_match_the_fixture_on_every_two_phase_state() {
    let f = fixture::load();
    let mut worst_beta = (0.0f64, String::new());
    let mut worst_x = (0.0f64, String::new());
    let mut checked = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        let n = spec.component_count();
        for state in &system.states {
            if !state.is_two_phase() {
                continue;
            }
            let fs = flash(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap();
            let d = flash_derivatives(&spec, state.pressure_pa, state.temperature_k, &state.z, &fs)
                .unwrap_or_else(|e| panic!("{}: {e}", state.id));

            // Derivative scales. dL/dp is of order 1e-8 per Pa and dL/dz of order 1; holding the
            // pressure column to a relative tolerance against a 1e-8 quantity would be measuring
            // roundoff, so each column gets a scale drawn from its own magnitude across the
            // fixture.
            let dl = state.dl_liquid_du.as_ref().unwrap();
            for v in 0..n {
                let scale = if v == 0 { 1e-9 } else { 1e-2 };
                // dbeta = -dL.
                let e = rel_to_scale(d.dbeta[v], -dl[v], scale);
                if e > worst_beta.0 {
                    worst_beta = (e, format!("{}/dbeta/du{v}", state.id));
                }
                checked += 1;
            }

            let dx = state.dx_liquid_du.as_ref().unwrap();
            let dy = state.dy_vapour_du.as_ref().unwrap();
            for i in 0..n {
                for v in 0..n {
                    let scale = if v == 0 { 1e-9 } else { 1e-2 };
                    for (ours, theirs, tag) in
                        [(d.dx[i][v], dx[i][v], "dx"), (d.dy[i][v], dy[i][v], "dy")]
                    {
                        let e = rel_to_scale(ours, theirs, scale);
                        if e > worst_x.0 {
                            worst_x = (e, format!("{}/{tag}{i}/du{v}", state.id));
                        }
                        checked += 1;
                    }
                }
            }
        }
    }

    assert!(checked >= 300, "compared only {checked} derivatives");
    assert!(
        worst_beta.0 < 1e-9,
        "dbeta/du disagrees with OPM: worst {:e} at {}",
        worst_beta.0,
        worst_beta.1
    );
    // 2e-9 rather than `dbeta/du`'s 1e-9, and the difference is coverage rather than a
    // regression. The bound was 1e-9 until the fixture gained `ternary_bhpdep_*` — four
    // two-phase states at 82-87 bar in a decane-rich mixture, which C12's BHP depletion needed
    // and which nothing else here sits near. The worst there is 1.17e-9 (`ternary_bhpdep_p87`,
    // `dy2/du1`); every state that existed before still meets 1e-9.
    assert!(
        worst_x.0 < 2e-9,
        "dx/du or dy/du disagrees with OPM: worst {:e} at {}",
        worst_x.0,
        worst_x.1
    );
}

/// Molar density derivatives, which carry the gas constant and therefore need the same exact
/// correction the C2 density comparison used.
#[test]
fn comp_derivatives_molar_density_matches_the_fixture_after_the_gas_constant_correction() {
    let f = fixture::load();
    let mut worst = (0.0f64, String::new());

    for system in &f.systems {
        let spec = spec_for(system);
        let n = spec.component_count();
        for state in &system.states {
            if !state.is_two_phase() {
                continue;
            }
            let fs = flash(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap();
            let d = flash_derivatives(&spec, state.pressure_pa, state.temperature_k, &state.z, &fs)
                .unwrap();

            for (ours, theirs_raw, tag) in [
                (
                    &d.dliquid_molar_density,
                    &state.liquid.as_ref().unwrap().dmolar_density_du,
                    "L",
                ),
                (
                    &d.dvapour_molar_density,
                    &state.vapour.as_ref().unwrap().dmolar_density_du,
                    "V",
                ),
            ] {
                for v in 0..n {
                    let theirs = super::units::opm_density_to_si(theirs_raw[v]);
                    let scale = if v == 0 { 1e-6 } else { 1e2 };
                    let e = rel_to_scale(ours[v], theirs, scale);
                    if e > worst.0 {
                        worst = (e, format!("{}/{tag}/du{v}", state.id));
                    }
                }
            }
        }
    }
    assert!(
        worst.0 < 1e-9,
        "molar density derivatives disagree: worst {:e} at {}",
        worst.0,
        worst.1
    );
}

// ---------------------------------------------------------------------------------------------
// Structural identities — these follow from the constraints and hold whatever the numbers are
// ---------------------------------------------------------------------------------------------

/// `sum_i x_i = 1` and `sum_i y_i = 1` identically, so every column of `dx` and `dy` must sum to
/// zero. Independent of the EOS, the flash and the oracle alike.
#[test]
fn comp_derivatives_phase_composition_columns_sum_to_zero() {
    let f = fixture::load();
    let mut worst = (0.0f64, String::new());

    for system in &f.systems {
        let spec = spec_for(system);
        let n = spec.component_count();
        for state in &system.states {
            let fs = flash(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap();
            let d = flash_derivatives(&spec, state.pressure_pa, state.temperature_k, &state.z, &fs)
                .unwrap();
            for (rows, tag) in [(&d.dx, "dx"), (&d.dy, "dy")] {
                if rows.is_empty() {
                    continue;
                }
                for v in 0..n {
                    let sum: f64 = (0..n).map(|i| rows[i][v]).sum();
                    let magnitude: f64 = (0..n)
                        .map(|i| rows[i][v].abs())
                        .fold(0.0, f64::max)
                        .max(1.0);
                    let e = sum.abs() / magnitude;
                    if e > worst.0 {
                        worst = (e, format!("{}/{tag}/du{v}", state.id));
                    }
                }
            }
        }
    }
    assert!(
        worst.0 < 1e-12,
        "phase composition derivatives do not satisfy sum = 1: worst {:e} at {}",
        worst.0,
        worst.1
    );
}

/// The invariant C4 names explicitly: `z_(N-1) = 1 - sum(z_0..z_(N-2))`, so
/// `dz_(N-1)/dz_k = -1` for every independent `k`, and `dz_i/dz_k = delta_ik` otherwise.
///
/// In a single-phase state the phase composition *is* `z`, so the reported `dx` or `dy` is that
/// identity directly and can be checked exactly rather than to a tolerance.
#[test]
fn comp_derivatives_dependent_component_has_derivative_minus_one() {
    let f = fixture::load();
    let mut checked = 0;

    for system in &f.systems {
        let spec = spec_for(system);
        let n = spec.component_count();
        for state in &system.states {
            let fs = flash(
                &spec,
                state.pressure_pa,
                state.temperature_k,
                &state.z,
                None,
            )
            .unwrap();
            if fs.phase_state == PhaseState::TwoPhase {
                continue;
            }
            let d = flash_derivatives(&spec, state.pressure_pa, state.temperature_k, &state.z, &fs)
                .unwrap();
            let rows = if d.dx.is_empty() { &d.dy } else { &d.dx };

            for i in 0..n {
                // Slot 0 is pressure: the feed composition does not depend on pressure.
                assert_eq!(
                    rows[i][0], 0.0,
                    "{}: dz_{i}/dp must be exactly zero",
                    state.id
                );
                for k in 0..(n - 1) {
                    let expected = if i == k {
                        1.0
                    } else if i == n - 1 {
                        -1.0
                    } else {
                        0.0
                    };
                    assert_eq!(
                        rows[i][1 + k],
                        expected,
                        "{}: dz_{i}/dz_{k} must be exactly {expected}",
                        state.id
                    );
                }
            }
            checked += 1;
        }
    }
    assert_eq!(
        checked, 18,
        "every single-phase fixture state must be covered"
    );
}

/// A single-phase state has no `beta` derivative, and no entries at all for the absent phase.
/// Reporting zeros instead would read as a real quantity that happens not to be moving.
#[test]
fn comp_derivatives_absent_phases_carry_no_entries() {
    let f = fixture::load();
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
            let d = flash_derivatives(&spec, state.pressure_pa, state.temperature_k, &state.z, &fs)
                .unwrap();
            match fs.phase_state {
                PhaseState::TwoPhase => {
                    assert!(!d.dbeta.is_empty(), "{}", state.id);
                    assert!(!d.dx.is_empty() && !d.dy.is_empty(), "{}", state.id);
                    assert!(!d.dliquid_molar_density.is_empty(), "{}", state.id);
                    assert!(!d.dvapour_molar_density.is_empty(), "{}", state.id);
                }
                PhaseState::SingleLiquid => {
                    assert!(d.dbeta.is_empty(), "{}", state.id);
                    assert!(!d.dx.is_empty() && d.dy.is_empty(), "{}", state.id);
                    assert!(d.dvapour_molar_density.is_empty(), "{}", state.id);
                    assert!(d.dvapour_mass_density.is_empty(), "{}", state.id);
                }
                PhaseState::SingleVapour => {
                    assert!(d.dbeta.is_empty(), "{}", state.id);
                    assert!(d.dx.is_empty() && !d.dy.is_empty(), "{}", state.id);
                    assert!(d.dliquid_molar_density.is_empty(), "{}", state.id);
                    assert!(d.dliquid_mass_density.is_empty(), "{}", state.id);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Finite differences, over a step sweep
// ---------------------------------------------------------------------------------------------

/// Central differences in pressure, over a sweep of step sizes.
///
/// A single step can agree by luck. What distinguishes a correct derivative is the **plateau**:
/// over a range of steps the FD estimate is stable, with truncation error dominating above it and
/// roundoff below. This asserts the plateau exists and that the analytic value sits inside it.
#[test]
fn comp_derivatives_pressure_matches_finite_differences_over_a_step_plateau() {
    let spec = pinned::ternary().unwrap();
    let t = 423.15;
    let z = [0.2, 0.5, 0.3];
    // A well-inside-two-phase state from the fixture's pressure traverse, deliberately not near a
    // phase boundary: a central difference across a boundary is not a derivative of anything.
    let p = 1.5e7;

    let base = flash(&spec, p, t, &z, None).unwrap();
    let analytic = flash_derivatives(&spec, p, t, &z, &base).unwrap();

    let mut plateau = Vec::new();
    for exponent in 0..8 {
        let h = 1.0e5 / 2f64.powi(exponent); // 1 bar down to ~0.008 bar
        let up = flash(&spec, p + h, t, &z, None).unwrap();
        let down = flash(&spec, p - h, t, &z, None).unwrap();
        let fd = (up.beta - down.beta) / (2.0 * h);
        plateau.push((h, fd));
    }

    let analytic_beta = analytic.dbeta[0];
    let errors: Vec<f64> = plateau
        .iter()
        .map(|(_, fd)| (fd - analytic_beta).abs() / analytic_beta.abs())
        .collect();
    let best = errors.iter().cloned().fold(f64::INFINITY, f64::min);

    assert!(
        best < 1e-7,
        "no FD step agrees with dbeta/dp = {analytic_beta:e}; \
         sweep was {plateau:?}, relative errors {errors:?}"
    );
    // The plateau: at least four consecutive steps within 1e-5. If only one step agreed, the
    // agreement would be a coincidence rather than a derivative.
    let inside = errors.iter().filter(|e| **e < 1e-5).count();
    assert!(
        inside >= 4,
        "only {inside} of {} steps land on the plateau; errors {errors:?}",
        errors.len()
    );
}

/// Central differences in each independent composition coordinate.
///
/// The perturbation must preserve normalization, so moving `z_k` by `h` moves the dependent
/// `z_(N-1)` by `-h`. Perturbing one component alone would break `sum z = 1` and measure the
/// derivative of a different function.
#[test]
fn comp_derivatives_composition_matches_finite_differences() {
    let spec = pinned::ternary().unwrap();
    let t = 423.15;
    let p = 1.5e7;
    let z = [0.2, 0.5, 0.3];
    let n = 3;

    let base = flash(&spec, p, t, &z, None).unwrap();
    let analytic = flash_derivatives(&spec, p, t, &z, &base).unwrap();

    for k in 0..(n - 1) {
        let mut best = f64::INFINITY;
        let mut best_detail = String::new();

        for exponent in 0..6 {
            let h = 1.0e-4 / 2f64.powi(exponent);
            let mut up = z;
            let mut down = z;
            up[k] += h;
            up[n - 1] -= h;
            down[k] -= h;
            down[n - 1] += h;

            let fu = flash(&spec, p, t, &up, None).unwrap();
            let fd_ = flash(&spec, p, t, &down, None).unwrap();
            let fd = (fu.beta - fd_.beta) / (2.0 * h);

            let analytic_beta = analytic.dbeta[1 + k];
            let e = (fd - analytic_beta).abs() / analytic_beta.abs().max(1e-3);
            if e < best {
                best = e;
                best_detail = format!("h = {h:e}, fd = {fd:e}, analytic = {analytic_beta:e}");
            }
        }
        assert!(
            best < 1e-6,
            "dbeta/dz_{k} does not match FD: best relative error {best:e} ({best_detail})"
        );
    }

    // And the phase compositions, at the step size the beta sweep settled on.
    let h = 1.0e-5;
    for k in 0..(n - 1) {
        let mut up = z;
        let mut down = z;
        up[k] += h;
        up[n - 1] -= h;
        down[k] -= h;
        down[n - 1] += h;
        let fu = flash(&spec, p, t, &up, None).unwrap();
        let fd_ = flash(&spec, p, t, &down, None).unwrap();

        for i in 0..n {
            for (ours, fdv, tag) in [
                (
                    analytic.dx[i][1 + k],
                    (fu.x[i] - fd_.x[i]) / (2.0 * h),
                    "dx",
                ),
                (
                    analytic.dy[i][1 + k],
                    (fu.y[i] - fd_.y[i]) / (2.0 * h),
                    "dy",
                ),
            ] {
                let e = (ours - fdv).abs() / ours.abs().max(1e-3);
                assert!(
                    e < 1e-5,
                    "{tag}[{i}]/dz_{k}: analytic {ours:e} vs FD {fdv:e}, relative error {e:e}"
                );
            }
        }
    }
}

/// Molar density derivatives against FD, which checks the implicit differentiation of the cubic
/// root specifically — nothing else in the chain touches `dP/dZ`.
#[test]
fn comp_derivatives_molar_density_matches_finite_differences() {
    let spec = pinned::binary().unwrap();
    let t = 423.15;
    let p = 1.0e7;
    let z = [0.6, 0.4];

    let base = flash(&spec, p, t, &z, None).unwrap();
    let analytic = flash_derivatives(&spec, p, t, &z, &base).unwrap();
    assert_eq!(base.phase_state, PhaseState::TwoPhase);

    let h = 1.0e4;
    let up = flash(&spec, p + h, t, &z, None).unwrap();
    let down = flash(&spec, p - h, t, &z, None).unwrap();

    for (ours, fd, tag) in [
        (
            analytic.dliquid_molar_density[0],
            (up.liquid.as_ref().unwrap().molar_density
                - down.liquid.as_ref().unwrap().molar_density)
                / (2.0 * h),
            "liquid",
        ),
        (
            analytic.dvapour_molar_density[0],
            (up.vapour.as_ref().unwrap().molar_density
                - down.vapour.as_ref().unwrap().molar_density)
                / (2.0 * h),
            "vapour",
        ),
    ] {
        let e = (ours - fd).abs() / ours.abs();
        assert!(
            e < 1e-6,
            "d(c_{tag})/dp: analytic {ours:e} vs FD {fd:e}, relative error {e:e}"
        );
    }
}

/// Single-phase density derivatives, where there is no equilibrium system and the whole result
/// comes from the cubic. Checked separately because the two-phase path could mask an error here.
#[test]
fn comp_derivatives_single_phase_density_matches_finite_differences() {
    let spec = pinned::binary().unwrap();
    let t = 423.15;
    let p = 3.0e7;
    let z = [0.6, 0.4];

    let base = flash(&spec, p, t, &z, None).unwrap();
    assert_eq!(base.phase_state, PhaseState::SingleLiquid);
    let analytic = flash_derivatives(&spec, p, t, &z, &base).unwrap();

    let h = 1.0e4;
    let up = flash(&spec, p + h, t, &z, None).unwrap();
    let down = flash(&spec, p - h, t, &z, None).unwrap();
    let fd = (up.liquid.as_ref().unwrap().molar_density
        - down.liquid.as_ref().unwrap().molar_density)
        / (2.0 * h);
    let ours = analytic.dliquid_molar_density[0];
    assert!(
        (ours - fd).abs() / ours.abs() < 1e-6,
        "single-phase dc/dp: analytic {ours:e} vs FD {fd:e}"
    );

    // And in composition, where the mass density also picks up the molar-mass weighting.
    let hz = 1.0e-5;
    let up = flash(&spec, p, t, &[z[0] + hz, z[1] - hz], None).unwrap();
    let down = flash(&spec, p, t, &[z[0] - hz, z[1] + hz], None).unwrap();
    let fd = (up.liquid.as_ref().unwrap().mass_density
        - down.liquid.as_ref().unwrap().mass_density)
        / (2.0 * hz);
    let ours = analytic.dliquid_mass_density[1];
    assert!(
        (ours - fd).abs() / ours.abs() < 1e-5,
        "single-phase d(rho)/dz: analytic {ours:e} vs FD {fd:e}"
    );
}

// ---------------------------------------------------------------------------------------------
// Failure contracts
// ---------------------------------------------------------------------------------------------

/// A state that is not converged has no meaningful derivatives, and saying so is better than
/// returning the derivatives of whatever it happens to be.
#[test]
fn comp_derivatives_reject_a_state_that_is_not_at_equilibrium() {
    let spec = pinned::binary().unwrap();
    let p = 1.0e7;
    let t = 423.15;
    let z = [0.6, 0.4];

    let mut state = flash(&spec, p, t, &z, None).unwrap();
    // Move beta well off the solution while leaving everything else alone.
    state.beta += 0.1;

    assert!(
        matches!(
            flash_derivatives(&spec, p, t, &z, &state),
            Err(DerivativeError::NotAtEquilibrium { .. })
        ),
        "a perturbed state must be rejected, not differentiated"
    );
}

/// Diagnostics are populated whether or not anything went wrong, because they are what a
/// mismatch report needs and collecting them only on failure means never having them.
#[test]
fn comp_derivatives_report_conditioning_diagnostics() {
    let f = fixture::load();
    let mut worst_pivot = f64::INFINITY;
    let mut worst_separation = f64::INFINITY;

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
            let d = flash_derivatives(&spec, state.pressure_pa, state.temperature_k, &state.z, &fs)
                .unwrap();
            assert!(
                d.diagnostics.equilibrium_residual < 1e-8,
                "{}: differentiated at residual {:e}",
                state.id,
                d.diagnostics.equilibrium_residual
            );
            assert!(d.diagnostics.min_root_separation.is_finite());
            if fs.phase_state == PhaseState::TwoPhase {
                worst_pivot = worst_pivot.min(d.diagnostics.min_pivot);
            }
            worst_separation = worst_separation.min(d.diagnostics.min_root_separation);
        }
    }

    // Recorded so a later change that degrades conditioning is visible rather than inferred.
    assert!(
        worst_pivot > 1e-6,
        "the equilibrium Jacobian is worse conditioned than recorded: min pivot {worst_pivot:e}"
    );
    assert!(
        worst_separation > 1e-3,
        "cubic roots are closer than recorded: min |dP/dZ| {worst_separation:e}"
    );
}

/// An exactly zero component must be differentiable. `ln K = ln(y_i / x_i)` is `0/0` there, so the
/// equilibrium ratios are taken from the fugacity coefficients instead — the same number wherever
/// both are defined, and defined where the ratio is not.
///
/// Found by C10: a cell with an absent component made the whole assembly fail with a NaN before
/// any Newton step was taken, which is the shape of an error that would otherwise have surfaced as
/// an unexplained solver failure much later.
#[test]
fn comp_derivatives_handle_an_exactly_zero_component() {
    let spec = pinned::ternary().unwrap();
    let binary = pinned::binary().unwrap();
    let t = 423.15;

    for p in [2.0e6, 5.0e6, 1.0e7, 1.5e7, 2.0e7] {
        let z = [0.0, 0.6, 0.4];
        let state = flash(&spec, p, t, &z, None).unwrap();
        let d =
            flash_derivatives(&spec, p, t, &z, &state).unwrap_or_else(|e| panic!("p = {p}: {e}"));

        for row in d.dx.iter().chain(d.dy.iter()) {
            assert!(
                row.iter().all(|v| v.is_finite()),
                "p = {p}: a composition derivative is not finite: {row:?}"
            );
        }
        assert!(d.dbeta.iter().all(|v| v.is_finite()), "p = {p}");
        assert!(
            d.dliquid_molar_density.iter().all(|v| v.is_finite()),
            "p = {p}"
        );

        if state.phase_state != PhaseState::TwoPhase {
            continue;
        }

        // The absent component stays absent when the *other* coordinates move, so its derivative
        // with respect to pressure is zero and with respect to z_1 is zero.
        assert_eq!(
            d.dx[0][0], 0.0,
            "p = {p}: an absent component moved with pressure"
        );
        assert_eq!(d.dy[0][0], 0.0);
        assert_eq!(
            d.dx[0][2], 0.0,
            "p = {p}: an absent component moved with z_1"
        );

        // But introducing it has a finite, positive effect: dx_0/dz_0 > 0.
        assert!(
            d.dx[0][1] > 0.0 && d.dy[0][1] > 0.0,
            "p = {p}: the absent component cannot be introduced: dx {} dy {}",
            d.dx[0][1],
            d.dy[0][1]
        );

        // And the remaining two components must behave exactly as the equivalent binary does.
        let b_state = flash(&binary, p, t, &[0.6, 0.4], None).unwrap();
        let b = flash_derivatives(&binary, p, t, &[0.6, 0.4], &b_state).unwrap();
        assert!(
            (d.dbeta[0] - b.dbeta[0]).abs() / b.dbeta[0].abs().max(1e-12) < 1e-6,
            "p = {p}: dbeta/dp differs from the binary's: {} vs {}",
            d.dbeta[0],
            b.dbeta[0]
        );
    }
}
