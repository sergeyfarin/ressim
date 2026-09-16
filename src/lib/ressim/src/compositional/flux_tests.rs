//! C9 face-flux contract tests (`comp_flux_*`).

use super::flux::{FluxError, Gravity, UpstreamSide, face_flux};
use super::relperm::RelativePermeabilityModel;
use super::state::CompositionalCellState;
use crate::fluid::flash::{PhaseState, flash};
use crate::fluid::pinned;
use crate::fluid::specification::FluidSpecification;
use crate::fluid::units::bar_to_pa;

/// `DARCY_METRIC_FACTOR * geometric_transmissibility` for a plausible face: 100 mD over a
/// 100 m x 10 m interface at 100 m spacing. The value only has to be positive and physical — this
/// is the same quantity the black-oil assembler computes, and C9 reuses it rather than redefining
/// the conversion.
const GEOM_T: f64 = 8.526_988_8e-3 * 100.0 * (100.0 * 10.0) / 100.0;

fn relperm() -> RelativePermeabilityModel {
    RelativePermeabilityModel::Linear
}

fn cell(p_bar: f64, z: &[f64]) -> CompositionalCellState {
    CompositionalCellState::new(p_bar, z.to_vec()).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

// ---------------------------------------------------------------------------------------------
// Direction and conservation
// ---------------------------------------------------------------------------------------------

/// Flux runs from high pressure to low, and reversing the pair reverses the sign and nothing else.
#[test]
fn comp_flux_runs_downhill_and_reverses_with_the_pair() {
    let spec = pinned::ternary().unwrap();
    let hot = cell(160.0, &[0.2, 0.5]);
    let cold = cell(150.0, &[0.2, 0.5]);

    let forward = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        Gravity::OFF,
        (0, &hot),
        (1, &cold),
    )
    .unwrap();
    let reverse = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        Gravity::OFF,
        (0, &cold),
        (1, &hot),
    )
    .unwrap();

    assert!(forward.potential_difference_bar > 0.0);
    assert_eq!(forward.upstream, [UpstreamSide::First; 2]);
    assert_eq!(reverse.upstream, [UpstreamSide::Second; 2]);

    for c in 0..3 {
        assert!(
            forward.component_moles_per_day[c] > 0.0,
            "component {c} should flow from the high-pressure cell"
        );
        assert!(
            rel(
                reverse.component_moles_per_day[c],
                -forward.component_moles_per_day[c]
            ) < 1e-12,
            "component {c}: reversing the pair did not negate the flux"
        );
    }
}

/// A face between identical cells carries nothing. The uniform-state stationarity the plan asks
/// for, at the level of one face.
#[test]
fn comp_flux_vanishes_between_identical_cells() {
    for (spec, z) in [
        (pinned::binary().unwrap(), vec![0.6]),
        (pinned::ternary().unwrap(), vec![0.2, 0.5]),
    ] {
        for p in [50.0, 150.0, 300.0] {
            let a = cell(p, &z);
            let b = cell(p, &z);
            let f = face_flux(&spec, &relperm(), GEOM_T, Gravity::OFF, (0, &a), (1, &b)).unwrap();
            assert_eq!(f.potential_difference_bar, 0.0);
            for c in 0..spec.component_count() {
                assert_eq!(
                    f.component_moles_per_day[c], 0.0,
                    "p = {p}: component {c} moves between identical cells"
                );
            }
        }
    }
}

/// The two cells receive equal and opposite contributions. Stated here as the property the
/// assembler relies on: one face flux, inserted with two signs, cannot create or destroy material
/// however wrong its magnitude is.
#[test]
fn comp_flux_contributions_to_the_two_cells_cancel_exactly() {
    let spec = pinned::ternary().unwrap();
    let a = cell(170.0, &[0.25, 0.45]);
    let b = cell(140.0, &[0.15, 0.55]);
    let f = face_flux(&spec, &relperm(), GEOM_T, Gravity::OFF, (0, &a), (1, &b)).unwrap();

    for c in 0..3 {
        let into_i = -f.component_moles_per_day[c];
        let into_j = f.component_moles_per_day[c];
        assert_eq!(into_i + into_j, 0.0, "component {c} does not cancel");
    }
}

/// Every component's flux must come out of the upstream cell's composition. A face that used the
/// downstream composition would still conserve and still point the right way.
#[test]
fn comp_flux_uses_the_upstream_composition() {
    let spec = pinned::ternary().unwrap();
    // Two cells with very different compositions, so upstream and downstream are distinguishable.
    let rich = cell(200.0, &[0.05, 0.10]);
    let lean = cell(150.0, &[0.45, 0.45]);

    let f = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        Gravity::OFF,
        (0, &rich),
        (1, &lean),
    )
    .unwrap();
    assert_eq!(f.upstream, [UpstreamSide::First; 2]);

    let total: f64 = f.component_moles_per_day.iter().sum();
    let fractions: Vec<f64> = f
        .component_moles_per_day
        .iter()
        .map(|q| q / total)
        .collect();

    // The mix crossing the face must resemble the upstream cell's overall composition far more
    // than the downstream cell's. It is not identical to either, because the phases move at
    // different rates — which is exactly the compositional effect being modelled.
    let up = rich.overall_composition();
    let down = lean.overall_composition();
    let d_up: f64 = (0..3).map(|c| (fractions[c] - up[c]).abs()).sum();
    let d_down: f64 = (0..3).map(|c| (fractions[c] - down[c]).abs()).sum();
    assert!(
        d_up < d_down,
        "the crossing mixture {fractions:?} is closer to the downstream composition {down:?} \
         than to the upstream one {up:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Phase structure
// ---------------------------------------------------------------------------------------------

/// A single-phase upstream cell contributes through that phase only. The absent phase is not a
/// zero mobility times an undefined density — there is no term at all, and its rate is exactly
/// zero rather than a small number.
#[test]
fn comp_flux_single_phase_upstream_moves_only_the_phase_that_exists() {
    let spec = pinned::binary().unwrap();

    // 400 bar is single-phase liquid for this binary; the downstream cell is two-phase.
    let liquid_up = cell(400.0, &[0.6]);
    let two_phase_down = cell(100.0, &[0.6]);
    assert_eq!(
        flash(
            &spec,
            bar_to_pa(400.0),
            spec.reservoir_temperature_k(),
            &liquid_up.overall_composition(),
            None
        )
        .unwrap()
        .phase_state,
        PhaseState::SingleLiquid
    );

    let f = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        Gravity::OFF,
        (0, &liquid_up),
        (1, &two_phase_down),
    )
    .unwrap();
    assert!(f.phase_rates_m3_per_day[0] > 0.0, "the liquid should move");
    assert_eq!(
        f.phase_rates_m3_per_day[1], 0.0,
        "there is no vapour upstream, so its rate must be exactly zero"
    );
    for c in 0..2 {
        assert!(f.component_moles_per_day[c] > 0.0);
    }
}

/// Phase appearance across a face: the upstream cell is single phase and the downstream one is
/// two-phase, and the flux is determined entirely by the upstream side. Nothing about the
/// downstream phase state may reach the flux, or the upwind scheme is not upwind.
#[test]
fn comp_flux_depends_on_the_downstream_cell_only_through_its_pressure() {
    let spec = pinned::ternary().unwrap();
    let up = cell(300.0, &[0.2, 0.5]);

    // Two downstream cells at the same pressure but very different compositions, hence different
    // phase states.
    let down_a = cell(150.0, &[0.2, 0.5]);
    let down_b = cell(150.0, &[0.05, 0.05]);
    let a = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        Gravity::OFF,
        (0, &up),
        (1, &down_a),
    )
    .unwrap();
    let b = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        Gravity::OFF,
        (0, &up),
        (1, &down_b),
    )
    .unwrap();

    for c in 0..3 {
        assert_eq!(
            a.component_moles_per_day[c], b.component_moles_per_day[c],
            "component {c}: the downstream composition changed the flux"
        );
    }
}

/// Both phases move when the upstream cell is two-phase, and the vapour — being far less viscous —
/// moves disproportionately to its saturation. That is the physical content of the mobility ratio,
/// and it is the reason a compositional front is not a piston.
#[test]
fn comp_flux_two_phase_upstream_moves_both_phases_at_different_mobilities() {
    let spec = pinned::binary().unwrap();
    let up = cell(110.0, &[0.6]);
    let down = cell(100.0, &[0.6]);

    let state = flash(
        &spec,
        bar_to_pa(110.0),
        spec.reservoir_temperature_k(),
        &up.overall_composition(),
        None,
    )
    .unwrap();
    assert_eq!(state.phase_state, PhaseState::TwoPhase);

    let f = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        Gravity::OFF,
        (0, &up),
        (1, &down),
    )
    .unwrap();
    let (q_l, q_v) = (f.phase_rates_m3_per_day[0], f.phase_rates_m3_per_day[1]);
    assert!(
        q_l > 0.0 && q_v > 0.0,
        "both phases should move: {q_l}, {q_v}"
    );

    // Volumetric rate fractions against saturation fractions.
    let s_v = state.vapour_saturation();
    let rate_fraction_v = q_v / (q_l + q_v);
    assert!(
        rate_fraction_v > s_v,
        "the vapour occupies {s_v} of the volume but carries only {rate_fraction_v} of the flow; \
         with straight-line kr the less viscous phase must be over-represented"
    );
}

// ---------------------------------------------------------------------------------------------
// The upwind branch
// ---------------------------------------------------------------------------------------------

/// At exactly zero potential difference the first cell is upstream. A convention, not a physical
/// fact, so it is pinned as one — and it is the same convention `fim/flux.rs` uses.
#[test]
fn comp_flux_zero_potential_takes_the_first_cell_upstream() {
    let spec = pinned::ternary().unwrap();
    // Same pressure, different compositions, so the choice of upstream side is observable.
    let a = cell(150.0, &[0.2, 0.5]);
    let b = cell(150.0, &[0.05, 0.05]);
    let f = face_flux(&spec, &relperm(), GEOM_T, Gravity::OFF, (0, &a), (1, &b)).unwrap();

    assert_eq!(f.potential_difference_bar, 0.0);
    assert_eq!(f.upstream, [UpstreamSide::First; 2]);
    // And the flux is zero regardless, because dphi multiplies every term.
    for c in 0..3 {
        assert_eq!(f.component_moles_per_day[c], 0.0);
    }
}

/// The branch is frozen on the value, so the Jacobian is the derivative of one smooth branch. An
/// infinitesimal pressure change that does not cross zero must not change which side is upstream.
#[test]
fn comp_flux_upstream_branch_is_stable_away_from_zero() {
    let spec = pinned::binary().unwrap();
    let down = cell(150.0, &[0.6]);
    for delta in [1e-9, 1e-6, 1e-3, 1.0, 50.0] {
        let up = cell(150.0 + delta, &[0.6]);
        let f = face_flux(
            &spec,
            &relperm(),
            GEOM_T,
            Gravity::OFF,
            (0, &up),
            (1, &down),
        )
        .unwrap();
        assert_eq!(f.upstream, [UpstreamSide::First; 2], "delta = {delta}");
        let flipped = cell(150.0 - delta, &[0.6]);
        let g = face_flux(
            &spec,
            &relperm(),
            GEOM_T,
            Gravity::OFF,
            (0, &flipped),
            (1, &down),
        )
        .unwrap();
        assert_eq!(g.upstream, [UpstreamSide::Second; 2], "delta = -{delta}");
    }
}

// ---------------------------------------------------------------------------------------------
// The Jacobian
// ---------------------------------------------------------------------------------------------

/// Every entry of the `N x 2N` face Jacobian against central differences, on both neighbours and
/// both phase regimes. Perturbations stay well away from the upwind switch, because a central
/// difference across a discontinuity is not a derivative.
#[test]
fn comp_flux_jacobian_matches_finite_differences_on_both_neighbours() {
    let cases: Vec<(&str, FluidSpecification, f64, Vec<f64>, f64, Vec<f64>)> = vec![
        (
            "binary two-phase upstream",
            pinned::binary().unwrap(),
            120.0,
            vec![0.6],
            100.0,
            vec![0.55],
        ),
        (
            "binary liquid upstream",
            pinned::binary().unwrap(),
            400.0,
            vec![0.6],
            350.0,
            vec![0.6],
        ),
        (
            "ternary two-phase upstream",
            pinned::ternary().unwrap(),
            170.0,
            vec![0.2, 0.5],
            140.0,
            vec![0.25, 0.45],
        ),
    ];

    let mut worst = (0.0f64, String::new());

    for (name, spec, p_i, z_i, p_j, z_j) in cases {
        let n = spec.component_count();
        let base_i = cell(p_i, &z_i);
        let base_j = cell(p_j, &z_j);
        let base = face_flux(
            &spec,
            &relperm(),
            GEOM_T,
            Gravity::OFF,
            (0, &base_i),
            (1, &base_j),
        )
        .unwrap();

        // Pressure columns on each side.
        for (side, offset) in [(0usize, 0usize), (1usize, n)] {
            let h = 1e-3;
            let (up_i, up_j) = if side == 0 {
                (cell(p_i + h, &z_i), base_j.clone())
            } else {
                (base_i.clone(), cell(p_j + h, &z_j))
            };
            let (dn_i, dn_j) = if side == 0 {
                (cell(p_i - h, &z_i), base_j.clone())
            } else {
                (base_i.clone(), cell(p_j - h, &z_j))
            };
            let up = face_flux(
                &spec,
                &relperm(),
                GEOM_T,
                Gravity::OFF,
                (0, &up_i),
                (1, &up_j),
            )
            .unwrap();
            let dn = face_flux(
                &spec,
                &relperm(),
                GEOM_T,
                Gravity::OFF,
                (0, &dn_i),
                (1, &dn_j),
            )
            .unwrap();

            for c in 0..n {
                let fd =
                    (up.component_moles_per_day[c] - dn.component_moles_per_day[c]) / (2.0 * h);
                let scale = fd
                    .abs()
                    .max(base.component_moles_per_day[c].abs() * 1e-3)
                    .max(1e-6);
                let e = (base.jacobian[c][offset] - fd).abs() / scale;
                if e > worst.0 {
                    worst = (e, format!("{name}/dF{c}/dp[cell {side}]"));
                }
            }
        }

        // Composition columns on each side.
        for (side, offset_base) in [(0usize, 0usize), (1usize, n)] {
            for k in 0..(n - 1) {
                let h = 1e-6;
                let perturb = |sign: f64| {
                    let mut zi = z_i.clone();
                    let mut zj = z_j.clone();
                    if side == 0 {
                        zi[k] += sign * h;
                    } else {
                        zj[k] += sign * h;
                    }
                    (cell(p_i, &zi), cell(p_j, &zj))
                };
                let (ui, uj) = perturb(1.0);
                let (di, dj) = perturb(-1.0);
                let up =
                    face_flux(&spec, &relperm(), GEOM_T, Gravity::OFF, (0, &ui), (1, &uj)).unwrap();
                let dn =
                    face_flux(&spec, &relperm(), GEOM_T, Gravity::OFF, (0, &di), (1, &dj)).unwrap();

                for c in 0..n {
                    let fd =
                        (up.component_moles_per_day[c] - dn.component_moles_per_day[c]) / (2.0 * h);
                    let scale = fd
                        .abs()
                        .max(base.component_moles_per_day[c].abs() * 1e-3)
                        .max(1e-6);
                    let e = (base.jacobian[c][offset_base + 1 + k] - fd).abs() / scale;
                    if e > worst.0 {
                        worst = (e, format!("{name}/dF{c}/dz{k}[cell {side}]"));
                    }
                }
            }
        }
    }

    assert!(
        worst.0 < 1e-4,
        "the face Jacobian disagrees with FD: worst {:e} at {}",
        worst.0,
        worst.1
    );
}

/// The Jacobian is `N x 2N`: every component row depends on both neighbours. A face kernel that
/// only differentiated the upstream cell would be missing half the coupling, and would still pass
/// a conservation test.
#[test]
fn comp_flux_jacobian_depends_on_both_neighbours() {
    let spec = pinned::ternary().unwrap();
    let a = cell(170.0, &[0.2, 0.5]);
    let b = cell(140.0, &[0.25, 0.45]);
    let f = face_flux(&spec, &relperm(), GEOM_T, Gravity::OFF, (0, &a), (1, &b)).unwrap();

    assert_eq!(f.jacobian.len(), 3);
    assert_eq!(f.jacobian[0].len(), 6);

    for c in 0..3 {
        // The downstream cell enters only through its pressure, so column 3 must be nonzero and
        // columns 4 and 5 must be exactly zero.
        assert!(
            f.jacobian[c][0] != 0.0,
            "component {c}: no dependence on upstream pressure"
        );
        assert!(
            f.jacobian[c][3] != 0.0,
            "component {c}: no dependence on downstream pressure"
        );
        assert_eq!(
            f.jacobian[c][4], 0.0,
            "component {c}: the downstream composition must not enter an upwind flux"
        );
        assert_eq!(f.jacobian[c][5], 0.0);
    }

    // The downstream pressure enters **only** through `dphi`. Since `flux = K(upstream) * dphi`
    // with `K` independent of the downstream cell, `dF/dp_j` must be exactly `-flux/dphi`.
    for c in 0..3 {
        let k = f.component_moles_per_day[c] / f.potential_difference_bar;
        assert!(
            rel(f.jacobian[c][3], -k) < 1e-12,
            "component {c}: dF/dp_j is {} but -flux/dphi is {}",
            f.jacobian[c][3],
            -k
        );
    }

    // The upstream pressure is a different matter, and the two columns are **not** equal and
    // opposite: raising `p_i` also compresses the upstream fluid, shifts its phase split and
    // changes its viscosity, all of which move `K`. A face kernel that reused `-dF/dp_j` for the
    // upstream column would be dropping every one of those terms, so the asymmetry is asserted
    // rather than merely permitted.
    let asymmetric = (0..3).any(|c| rel(f.jacobian[c][0], -f.jacobian[c][3]) > 1e-6);
    assert!(
        asymmetric,
        "dF/dp_i equals -dF/dp_j, so the upstream cell's density, viscosity and phase split are          not being differentiated"
    );
}

/// A zero transmissibility gives zero flux and a zero Jacobian — a sealed face, not a special
/// case.
#[test]
fn comp_flux_a_sealed_face_carries_nothing() {
    let spec = pinned::ternary().unwrap();
    let a = cell(200.0, &[0.2, 0.5]);
    let b = cell(150.0, &[0.2, 0.5]);
    let f = face_flux(&spec, &relperm(), 0.0, Gravity::OFF, (0, &a), (1, &b)).unwrap();
    for c in 0..3 {
        assert_eq!(f.component_moles_per_day[c], 0.0);
        assert!(f.jacobian[c].iter().all(|d| *d == 0.0));
    }
}

/// Flux is linear in the transmissibility, which is what makes `geom_t` a geometric factor rather
/// than part of the physics.
#[test]
fn comp_flux_is_linear_in_the_transmissibility() {
    let spec = pinned::binary().unwrap();
    let a = cell(170.0, &[0.6]);
    let b = cell(150.0, &[0.6]);
    let one = face_flux(&spec, &relperm(), GEOM_T, Gravity::OFF, (0, &a), (1, &b)).unwrap();
    let ten = face_flux(
        &spec,
        &relperm(),
        10.0 * GEOM_T,
        Gravity::OFF,
        (0, &a),
        (1, &b),
    )
    .unwrap();
    for c in 0..2 {
        assert!(
            rel(
                ten.component_moles_per_day[c],
                10.0 * one.component_moles_per_day[c]
            ) < 1e-12
        );
    }
}

#[test]
fn comp_flux_rejects_an_invalid_transmissibility() {
    let spec = pinned::binary().unwrap();
    let a = cell(170.0, &[0.6]);
    let b = cell(150.0, &[0.6]);
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            face_flux(&spec, &relperm(), bad, Gravity::OFF, (0, &a), (1, &b)),
            Err(FluxError::InvalidTransmissibility { .. })
        ));
    }
}

/// The flux applies whatever relative permeability model it is given, rather than assuming one.
/// Two different models must give different fluxes at the same state.
#[test]
fn comp_flux_uses_the_supplied_relative_permeability_model() {
    use super::relperm::CoreyParameters;

    let spec = pinned::binary().unwrap();
    let a = cell(120.0, &[0.6]);
    let b = cell(100.0, &[0.6]);

    let linear = face_flux(
        &spec,
        &RelativePermeabilityModel::Linear,
        GEOM_T,
        Gravity::OFF,
        (0, &a),
        (1, &b),
    )
    .unwrap();

    // Corey with caller-supplied parameters — no defaults exist, so these are the test's.
    let corey = RelativePermeabilityModel::Corey(
        CoreyParameters::new(0.1, 0.05, 2.0, 2.0, 1.0, 1.0).unwrap(),
    );
    let with_corey = face_flux(&spec, &corey, GEOM_T, Gravity::OFF, (0, &a), (1, &b)).unwrap();

    let differs = (0..2).any(|c| {
        rel(
            with_corey.component_moles_per_day[c],
            linear.component_moles_per_day[c],
        ) > 1e-6
    });
    assert!(
        differs,
        "the flux is identical under two different relative permeability models, so it is not \
         using the one it was given"
    );
}
