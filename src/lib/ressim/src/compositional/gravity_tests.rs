//! C9 gravity subtask contract tests (`comp_gravity_*`).
//!
//! The plan requires a **sourced single-phase hydrostatic equilibrium** to pass before any
//! multiphase gravity case is trusted, and that is the order these run in. The oracle is analytic
//! rather than a fixture: at equilibrium a single-phase column satisfies
//!
//! ```text
//! dp/dz = rho g          so     p_i - p_j = rho_avg * 9.80665 * (depth_i - depth_j) * 1e-5
//! ```
//!
//! which is a statement about the physics and not about this implementation. It is the same
//! constant and the same sign convention `fim/flux.rs::gravity_head_generic` uses, so the two
//! models agree about which way is down.

use super::accumulation::cell_inventory;
use super::assembly::{Face, assemble};
use super::flux::{Gravity, STANDARD_GRAVITY, UpstreamSide, face_flux};
use super::layout::CompositionalLayout;
use super::relperm::RelativePermeabilityModel;
use super::state::{CompositionalCellState, CompositionalState, RockView};
use crate::fluid::flash::{PhaseState, flash};
use crate::fluid::pinned;
use crate::fluid::units::bar_to_pa;

fn relperm() -> RelativePermeabilityModel {
    RelativePermeabilityModel::Linear
}
const GEOM_T: f64 = 8.526_988_8e-3 * 100.0 * (100.0 * 10.0) / 100.0;

fn cell(p_bar: f64, z: &[f64]) -> CompositionalCellState {
    CompositionalCellState::new(p_bar, z.to_vec()).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

/// Gravity off is exactly off: every quantity is bit-identical to the pre-gravity path.
#[test]
fn comp_gravity_off_is_exactly_off() {
    let spec = pinned::ternary().unwrap();
    let a = cell(170.0, &[0.2, 0.5]);
    let b = cell(150.0, &[0.2, 0.5]);

    let without = face_flux(&spec, &relperm(), GEOM_T, Gravity::OFF, (0, &a), (1, &b)).unwrap();
    // A disabled gravity with a large depth difference must change nothing at all.
    let disabled = Gravity {
        enabled: false,
        depth_i_m: 0.0,
        depth_j_m: 500.0,
    };
    let with_disabled = face_flux(&spec, &relperm(), GEOM_T, disabled, (0, &a), (1, &b)).unwrap();
    assert_eq!(without, with_disabled);

    // And equal depths with gravity enabled is the same as gravity off, because the head is zero.
    let level = Gravity::between(2000.0, 2000.0);
    let with_level = face_flux(&spec, &relperm(), GEOM_T, level, (0, &a), (1, &b)).unwrap();
    assert_eq!(
        without.component_moles_per_day,
        with_level.component_moles_per_day
    );
}

/// **The gate the plan requires first.** A single-phase column at hydrostatic equilibrium carries
/// no flux, with the equilibrium pressure computed analytically from `dp/dz = rho g` rather than
/// from anything this module produced.
#[test]
fn comp_gravity_single_phase_hydrostatic_column_is_stationary() {
    let spec = pinned::binary().unwrap();
    let z = [0.6];
    let t = spec.reservoir_temperature_k();

    // 400 bar is single-phase liquid for this binary.
    let p_top = 400.0;
    let depth_top = 2000.0;
    let depth_bottom = 2010.0;

    let top = cell(p_top, &z);
    let top_state = flash(&spec, bar_to_pa(p_top), t, &top.overall_composition(), None).unwrap();
    assert_eq!(top_state.phase_state, PhaseState::SingleLiquid);

    // Solve p_bottom such that the head balances, iterating because rho depends on p. Two or three
    // passes are plenty over 10 m.
    let mut p_bottom = p_top;
    for _ in 0..40 {
        let bottom = cell(p_bottom, &z);
        let bottom_state = flash(
            &spec,
            bar_to_pa(p_bottom),
            t,
            &bottom.overall_composition(),
            None,
        )
        .unwrap();
        let rho_avg = 0.5
            * (top_state.liquid.as_ref().unwrap().mass_density
                + bottom_state.liquid.as_ref().unwrap().mass_density);
        // dp/dz = rho g, in bar over metres.
        p_bottom = p_top + rho_avg * STANDARD_GRAVITY * (depth_bottom - depth_top) * 1e-5;
    }

    let bottom = cell(p_bottom, &z);
    // Cell i is the deeper one, so depth_i > depth_j.
    let gravity = Gravity::between(depth_bottom, depth_top);
    let f = face_flux(&spec, &relperm(), GEOM_T, gravity, (0, &bottom), (1, &top)).unwrap();

    // The phase potential must vanish, and with it the flux.
    assert!(
        f.phase_potential_bar[0].abs() < 1e-10,
        "the liquid potential at hydrostatic equilibrium is {:e} bar",
        f.phase_potential_bar[0]
    );
    for c in 0..2 {
        assert!(
            f.component_moles_per_day[c].abs() < 1e-8,
            "component {c} moves at hydrostatic equilibrium: {} mol/day",
            f.component_moles_per_day[c]
        );
    }

    // And the deeper cell really is at higher pressure — the sign convention, stated as physics.
    assert!(
        p_bottom > p_top,
        "the deeper cell must be at higher pressure"
    );
    let expected_gradient =
        top_state.liquid.as_ref().unwrap().mass_density * STANDARD_GRAVITY * 1e-5;
    let actual_gradient = (p_bottom - p_top) / (depth_bottom - depth_top);
    assert!(
        rel(actual_gradient, expected_gradient) < 1e-3,
        "the hydrostatic gradient is {actual_gradient} bar/m, analytically {expected_gradient}"
    );
}

/// Away from equilibrium, gravity drives flow in the direction it should: an over-pressured deep
/// cell pushes up, an under-pressured one lets fluid fall.
#[test]
fn comp_gravity_drives_flow_toward_equilibrium() {
    let spec = pinned::binary().unwrap();
    let z = [0.6];
    let gravity = Gravity::between(2010.0, 2000.0); // cell i is deeper

    // Equal pressures: the deeper cell is under-pressured relative to hydrostatic, so fluid must
    // fall *into* it — flux from j to i, i.e. negative.
    let deep = cell(400.0, &z);
    let shallow = cell(400.0, &z);
    let f = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        gravity,
        (0, &deep),
        (1, &shallow),
    )
    .unwrap();
    assert!(
        f.phase_potential_bar[0] < 0.0,
        "with equal pressures the deeper cell must be under-pressured: {:e}",
        f.phase_potential_bar[0]
    );
    assert_eq!(f.upstream[0], UpstreamSide::Second);
    for c in 0..2 {
        assert!(
            f.component_moles_per_day[c] < 0.0,
            "component {c} should flow downward"
        );
    }

    // Over-pressure the deep cell far past hydrostatic and the flow reverses.
    let over = cell(420.0, &z);
    let g = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        gravity,
        (0, &over),
        (1, &shallow),
    )
    .unwrap();
    assert!(g.phase_potential_bar[0] > 0.0);
    assert_eq!(g.upstream[0], UpstreamSide::First);
    for c in 0..2 {
        assert!(
            g.component_moles_per_day[c] > 0.0,
            "component {c} should flow upward"
        );
    }
}

/// The multiphase case, and the reason the upstream side is per phase: with gravity on, the dense
/// liquid and the light vapour can have opposite potentials across the same face. A face that
/// picked one upstream side for both would move material the wrong way.
#[test]
fn comp_gravity_phases_can_flow_in_opposite_directions() {
    let spec = pinned::binary().unwrap();
    let z = [0.6];
    let t = spec.reservoir_temperature_k();

    // A two-phase state, so both densities exist and differ by an order of magnitude.
    let p = 100.0;
    let state = flash(
        &spec,
        bar_to_pa(p),
        t,
        &cell(p, &z).overall_composition(),
        None,
    )
    .unwrap();
    assert_eq!(state.phase_state, PhaseState::TwoPhase);
    let rho_l = state.liquid.as_ref().unwrap().mass_density;
    let rho_v = state.vapour.as_ref().unwrap().mass_density;
    assert!(
        rho_l > 5.0 * rho_v,
        "the densities must differ enough: {rho_l} vs {rho_v}"
    );

    // Choose a depth separation and a pressure difference that sits between the two hydrostatic
    // gradients, so the liquid wants to fall and the vapour wants to rise.
    let dz = 100.0;
    let head_liquid = rho_l * STANDARD_GRAVITY * dz * 1e-5;
    let head_vapour = rho_v * STANDARD_GRAVITY * dz * 1e-5;
    assert!(head_liquid > head_vapour);
    let dp = 0.5 * (head_liquid + head_vapour);

    let deep = cell(p + dp, &z);
    let shallow = cell(p, &z);
    let gravity = Gravity::between(2000.0 + dz, 2000.0);
    let f = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        gravity,
        (0, &deep),
        (1, &shallow),
    )
    .unwrap();

    assert!(
        f.phase_potential_bar[0] < 0.0,
        "the liquid should still be falling: {:e}",
        f.phase_potential_bar[0]
    );
    assert!(
        f.phase_potential_bar[1] > 0.0,
        "the vapour should be rising: {:e}",
        f.phase_potential_bar[1]
    );
    assert_eq!(
        f.upstream,
        [UpstreamSide::Second, UpstreamSide::First],
        "the two phases must be drawn from opposite cells"
    );
    assert!(
        f.phase_rates_m3_per_day[0] < 0.0,
        "the liquid rate should be downward"
    );
    assert!(
        f.phase_rates_m3_per_day[1] > 0.0,
        "the vapour rate should be upward"
    );
}

/// The head uses **mass** density, not molar. The two differ by roughly the molar mass, so the
/// wrong one would give a head three orders of magnitude out — and would still have the right
/// sign, which is why this is checked against an analytic value rather than a sign.
#[test]
fn comp_gravity_head_uses_mass_density_not_molar() {
    let spec = pinned::binary().unwrap();
    let z = [0.6];
    let t = spec.reservoir_temperature_k();
    let p = 400.0;

    let state = flash(
        &spec,
        bar_to_pa(p),
        t,
        &cell(p, &z).overall_composition(),
        None,
    )
    .unwrap();
    let rho_mass = state.liquid.as_ref().unwrap().mass_density;
    let rho_molar = state.liquid.as_ref().unwrap().molar_density;
    assert!(
        rho_molar > 10.0 * rho_mass,
        "the two must be distinguishable here"
    );

    let dz = 50.0;
    let deep = cell(p, &z);
    let shallow = cell(p, &z);
    let gravity = Gravity::between(2000.0 + dz, 2000.0);
    let f = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        gravity,
        (0, &deep),
        (1, &shallow),
    )
    .unwrap();

    // With equal pressures, the potential is exactly minus the head.
    let expected = -rho_mass * STANDARD_GRAVITY * dz * 1e-5;
    assert!(
        rel(f.phase_potential_bar[0], expected) < 1e-9,
        "the head is {} bar, analytically {expected} from the MASS density",
        f.phase_potential_bar[0]
    );
}

/// A phase present in only one cell contributes its own density to the head rather than being
/// averaged against a number that describes nothing.
#[test]
fn comp_gravity_uses_a_single_sided_density_when_a_phase_is_absent() {
    let spec = pinned::binary().unwrap();
    let z = [0.6];
    let t = spec.reservoir_temperature_k();

    // Deep cell single-phase liquid, shallow cell two-phase.
    let deep = cell(400.0, &z);
    let shallow = cell(100.0, &z);
    let deep_state = flash(
        &spec,
        bar_to_pa(400.0),
        t,
        &deep.overall_composition(),
        None,
    )
    .unwrap();
    let shallow_state = flash(
        &spec,
        bar_to_pa(100.0),
        t,
        &shallow.overall_composition(),
        None,
    )
    .unwrap();
    assert_eq!(deep_state.phase_state, PhaseState::SingleLiquid);
    assert_eq!(shallow_state.phase_state, PhaseState::TwoPhase);

    let dz = 100.0;
    let gravity = Gravity::between(2000.0 + dz, 2000.0);
    let f = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        gravity,
        (0, &deep),
        (1, &shallow),
    )
    .unwrap();

    // The vapour exists only in the shallow cell, so its head uses that density alone.
    let rho_v = shallow_state.vapour.as_ref().unwrap().mass_density;
    let expected_vapour_head = rho_v * STANDARD_GRAVITY * dz * 1e-5;
    let dp = 400.0 - 100.0;
    assert!(
        rel(f.phase_potential_bar[1], dp - expected_vapour_head) < 1e-9,
        "the vapour potential is {}, expected {} from the shallow cell's density alone",
        f.phase_potential_bar[1],
        dp - expected_vapour_head
    );

    // The liquid exists in both, so its head uses the average.
    let rho_l_avg = 0.5
        * (deep_state.liquid.as_ref().unwrap().mass_density
            + shallow_state.liquid.as_ref().unwrap().mass_density);
    assert!(
        rel(
            f.phase_potential_bar[0],
            dp - rho_l_avg * STANDARD_GRAVITY * dz * 1e-5
        ) < 1e-9
    );
}

/// The Jacobian with gravity on still matches finite differences, including the new coupling: the
/// head depends on **both** cells' densities, so the downstream cell's composition now enters a
/// flux it did not enter before.
#[test]
fn comp_gravity_jacobian_matches_finite_differences() {
    let spec = pinned::ternary().unwrap();
    let z_i = vec![0.2, 0.5];
    let z_j = vec![0.25, 0.45];
    let p_i = 170.0;
    let p_j = 150.0;
    let gravity = Gravity::between(2050.0, 2000.0);

    let base_i = cell(p_i, &z_i);
    let base_j = cell(p_j, &z_j);
    let base = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        gravity,
        (0, &base_i),
        (1, &base_j),
    )
    .unwrap();

    let mut worst = (0.0f64, String::new());
    let n = 3;

    for (side, offset) in [(0usize, 0usize), (1usize, n)] {
        // Pressure.
        let h = 1e-3;
        let step = |sign: f64| {
            if side == 0 {
                (cell(p_i + sign * h, &z_i), base_j.clone())
            } else {
                (base_i.clone(), cell(p_j + sign * h, &z_j))
            }
        };
        let (ui, uj) = step(1.0);
        let (di, dj) = step(-1.0);
        let up = face_flux(&spec, &relperm(), GEOM_T, gravity, (0, &ui), (1, &uj)).unwrap();
        let dn = face_flux(&spec, &relperm(), GEOM_T, gravity, (0, &di), (1, &dj)).unwrap();
        for c in 0..n {
            let fd = (up.component_moles_per_day[c] - dn.component_moles_per_day[c]) / (2.0 * h);
            let scale = fd
                .abs()
                .max(base.component_moles_per_day[c].abs() * 1e-3)
                .max(1e-6);
            let e = (base.jacobian[c][offset] - fd).abs() / scale;
            if e > worst.0 {
                worst = (e, format!("dF{c}/dp[cell {side}]"));
            }
        }

        // Composition.
        for k in 0..(n - 1) {
            let hz = 1e-6;
            let step = |sign: f64| {
                let mut zi = z_i.clone();
                let mut zj = z_j.clone();
                if side == 0 {
                    zi[k] += sign * hz;
                } else {
                    zj[k] += sign * hz;
                }
                (cell(p_i, &zi), cell(p_j, &zj))
            };
            let (ui, uj) = step(1.0);
            let (di, dj) = step(-1.0);
            let up = face_flux(&spec, &relperm(), GEOM_T, gravity, (0, &ui), (1, &uj)).unwrap();
            let dn = face_flux(&spec, &relperm(), GEOM_T, gravity, (0, &di), (1, &dj)).unwrap();
            for c in 0..n {
                let fd =
                    (up.component_moles_per_day[c] - dn.component_moles_per_day[c]) / (2.0 * hz);
                let scale = fd
                    .abs()
                    .max(base.component_moles_per_day[c].abs() * 1e-3)
                    .max(1e-6);
                let e = (base.jacobian[c][offset + 1 + k] - fd).abs() / scale;
                if e > worst.0 {
                    worst = (e, format!("dF{c}/dz{k}[cell {side}]"));
                }
            }
        }
    }

    assert!(
        worst.0 < 1e-4,
        "the gravity face Jacobian disagrees with FD: worst {:e} at {}",
        worst.0,
        worst.1
    );
}

/// With gravity on, the downstream cell's composition **does** enter the flux, through the head.
/// That is the structural difference from the zero-gravity case, where it was exactly zero, and it
/// is asserted so a regression to the old shape is caught.
#[test]
fn comp_gravity_couples_the_downstream_composition() {
    let spec = pinned::ternary().unwrap();
    let a = cell(170.0, &[0.2, 0.5]);
    let b = cell(150.0, &[0.25, 0.45]);

    let without = face_flux(&spec, &relperm(), GEOM_T, Gravity::OFF, (0, &a), (1, &b)).unwrap();
    let with = face_flux(
        &spec,
        &relperm(),
        GEOM_T,
        Gravity::between(2050.0, 2000.0),
        (0, &a),
        (1, &b),
    )
    .unwrap();

    for c in 0..3 {
        assert_eq!(
            without.jacobian[c][4], 0.0,
            "zero gravity must not couple z_0 downstream"
        );
        assert!(
            with.jacobian[c][4] != 0.0,
            "component {c}: gravity must couple the downstream composition through the head"
        );
    }
}

/// A gravity-equilibrated single-phase column assembles to zero residual everywhere — the
/// hydrostatic gate at grid level rather than face level.
#[test]
fn comp_gravity_hydrostatic_column_assembles_to_zero() {
    let spec = pinned::binary().unwrap();
    let z = vec![0.6];
    let t = spec.reservoir_temperature_k();
    let cells = 4;
    let dz = 5.0;
    let p_top = 400.0;

    // Build the column top-down, solving each cell's pressure from the one above.
    let mut pressures = vec![p_top];
    for _ in 1..cells {
        let above = *pressures.last().unwrap();
        let mut below = above;
        for _ in 0..40 {
            let rho_above = flash(
                &spec,
                bar_to_pa(above),
                t,
                &cell(above, &z).overall_composition(),
                None,
            )
            .unwrap()
            .liquid
            .as_ref()
            .unwrap()
            .mass_density;
            let rho_below = flash(
                &spec,
                bar_to_pa(below),
                t,
                &cell(below, &z).overall_composition(),
                None,
            )
            .unwrap()
            .liquid
            .as_ref()
            .unwrap()
            .mass_density;
            below = above + 0.5 * (rho_above + rho_below) * STANDARD_GRAVITY * dz * 1e-5;
        }
        pressures.push(below);
    }

    let layout = CompositionalLayout::new(2, cells, 0, 0).unwrap();
    let state =
        CompositionalState::new(&layout, pressures.iter().map(|p| cell(*p, &z)).collect()).unwrap();

    // Cell 0 is shallowest; face i is the upper cell, j the lower, so depth_i < depth_j.
    let faces: Vec<Face> = (0..cells - 1)
        .map(|i| Face {
            cell_i: i,
            cell_j: i + 1,
            geom_t: GEOM_T,
            gravity: Gravity::between(2000.0 + dz * i as f64, 2000.0 + dz * (i + 1) as f64),
        })
        .collect();

    let pore_volumes = vec![1000.0; cells];
    let rock = RockView {
        pore_volume_ref_m3: &pore_volumes,
        reference_pressure_bar: 200.0,
        compressibility_per_bar: 4.0e-5,
    };
    let previous: Vec<Vec<f64>> = (0..cells)
        .map(|c| {
            cell_inventory(&spec, &rock, c, state.cell(c))
                .unwrap()
                .0
                .component_moles
        })
        .collect();

    let result = assemble(
        &spec,
        &layout,
        &rock,
        &relperm(),
        &state,
        &faces,
        &previous,
        &vec![vec![0.0; 2]; cells],
        1.0,
    )
    .unwrap();

    let norm = result.scaled_residual_norm(&layout);
    assert!(
        norm < 1e-12,
        "a hydrostatic single-phase column is not stationary: scaled residual {norm:e}"
    );
}
