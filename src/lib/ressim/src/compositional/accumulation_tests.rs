//! C8 accumulation and scaling contract tests (`comp_accumulation_*`, `comp_scaling_*`).

use super::accumulation::{
    AccumulationError, EquationScaling, TRACE_COMPONENT_FLOOR, cell_accumulation, cell_inventory,
};
use super::state::{CompositionalCellState, RockView};
use crate::fluid::flash::{PhaseState, flash};
use crate::fluid::pinned;
use crate::fluid::specification::FluidSpecification;
use crate::fluid::units::bar_to_pa;

const PV_REF: [f64; 3] = [1000.0, 2000.0, 500.0];
const P_REF_BAR: f64 = 200.0;
const C_ROCK: f64 = 4.0e-5;

fn rock() -> RockView<'static> {
    RockView {
        pore_volume_ref_m3: &PV_REF,
        reference_pressure_bar: P_REF_BAR,
        compressibility_per_bar: C_ROCK,
    }
}

fn cell(p_bar: f64, z: &[f64]) -> CompositionalCellState {
    CompositionalCellState::new(p_bar, z.to_vec()).unwrap()
}

/// `(spec, cell)` pairs covering all three phase states, so every test that sweeps them is
/// exercising the single-phase branches and not only the two-phase one.
fn regimes() -> Vec<(&'static str, FluidSpecification, CompositionalCellState)> {
    vec![
        (
            "binary two-phase",
            pinned::binary().unwrap(),
            cell(100.0, &[0.6]),
        ),
        (
            "binary liquid",
            pinned::binary().unwrap(),
            cell(300.0, &[0.6]),
        ),
        (
            "ternary two-phase",
            pinned::ternary().unwrap(),
            cell(150.0, &[0.2, 0.5]),
        ),
        (
            "ternary liquid",
            pinned::ternary().unwrap(),
            cell(400.0, &[0.2, 0.5]),
        ),
        (
            "ternary vapour",
            pinned::ternary().unwrap(),
            cell(150.0, &[0.1, 0.891]),
        ),
    ]
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

// ---------------------------------------------------------------------------------------------
// Inventory
// ---------------------------------------------------------------------------------------------

/// The sum-over-phases inventory must equal `PV z_i / v_mix`, which follows from
/// `(1-beta) x_i + beta y_i = z_i`. Two expressions of the same quantity, one implemented and one
/// not, so agreement is evidence rather than a tautology.
#[test]
fn comp_accumulation_inventory_matches_the_independent_identity() {
    let rock = rock();
    let mut worst = (0.0f64, "");
    let mut regimes_seen = std::collections::BTreeSet::new();

    for (name, spec, c) in regimes() {
        let (inv, state) = cell_inventory(&spec, &rock, 0, &c).unwrap();
        regimes_seen.insert(format!("{:?}", inv.phase_state));
        let z = c.overall_composition();

        for i in 0..spec.component_count() {
            let independent = inv.pore_volume_m3 * z[i] / inv.mixture_molar_volume;
            let e = rel(inv.component_moles[i], independent);
            if e > worst.0 {
                worst = (e, name);
            }
        }
        // And the total.
        let total: f64 = inv.component_moles.iter().sum();
        assert!(
            rel(total, inv.total_moles) < 1e-12,
            "{name}: component moles sum to {total}, total_moles is {}",
            inv.total_moles
        );
        assert_eq!(inv.phase_state, state.phase_state);
    }

    assert_eq!(
        regimes_seen.len(),
        3,
        "the regimes fixture stopped covering all three phase states"
    );
    assert!(
        worst.0 < 1e-12,
        "the phase-sum and identity forms disagree: worst relative error {:e} at {}",
        worst.0,
        worst.1
    );
}

/// An independent inventory calculation, done from first principles rather than by rearranging the
/// implementation: take the cell's volume, split it by saturation, and count moles in each part.
#[test]
fn comp_accumulation_inventory_matches_a_first_principles_count() {
    let rock = rock();
    let spec = pinned::ternary().unwrap();
    let c = cell(150.0, &[0.2, 0.5]);

    let (inv, state) = cell_inventory(&spec, &rock, 1, &c).unwrap();
    assert_eq!(state.phase_state, PhaseState::TwoPhase);

    let pv = PV_REF[1] * ((150.0 - P_REF_BAR) * C_ROCK).exp();
    assert!(rel(inv.pore_volume_m3, pv) < 1e-15);

    // Volume occupied by each phase, then moles in each, then components in each.
    let v_vapour = pv * state.vapour_saturation();
    let v_liquid = pv - v_vapour;
    let moles_vapour = v_vapour * state.vapour.as_ref().unwrap().molar_density;
    let moles_liquid = v_liquid * state.liquid.as_ref().unwrap().molar_density;

    for i in 0..3 {
        let expected = moles_liquid * state.x[i] + moles_vapour * state.y[i];
        assert!(
            rel(inv.component_moles[i], expected) < 1e-12,
            "component {i}: {} vs first-principles {expected}",
            inv.component_moles[i]
        );
    }

    // The mole split must also reproduce beta, which is the mole fraction rather than the volume
    // fraction — a different quantity, and the one the two are easy to confuse.
    let beta_from_moles = moles_vapour / (moles_liquid + moles_vapour);
    assert!(rel(beta_from_moles, state.beta) < 1e-12);
}

/// Pore volume must compact and dilate with pressure, and the inventory must follow it.
#[test]
fn comp_accumulation_inventory_follows_the_rock_compressibility() {
    let rock = rock();
    let spec = pinned::binary().unwrap();

    let low = cell_inventory(&spec, &rock, 0, &cell(P_REF_BAR - 50.0, &[0.6]))
        .unwrap()
        .0;
    let at_ref = cell_inventory(&spec, &rock, 0, &cell(P_REF_BAR, &[0.6]))
        .unwrap()
        .0;
    let high = cell_inventory(&spec, &rock, 0, &cell(P_REF_BAR + 50.0, &[0.6]))
        .unwrap()
        .0;

    assert_eq!(at_ref.pore_volume_m3, PV_REF[0]);
    assert!(low.pore_volume_m3 < at_ref.pore_volume_m3);
    assert!(high.pore_volume_m3 > at_ref.pore_volume_m3);
    // And more moles fit in at higher pressure: the fluid compresses faster than the rock.
    assert!(high.total_moles > at_ref.total_moles);
    assert!(low.total_moles < at_ref.total_moles);
}

// ---------------------------------------------------------------------------------------------
// Residual
// ---------------------------------------------------------------------------------------------

/// A closed cell whose state has not changed has exactly zero residual. Nothing else about the
/// discretization matters if this is not true.
#[test]
fn comp_accumulation_a_stationary_closed_cell_has_zero_residual() {
    let rock = rock();
    for (name, spec, c) in regimes() {
        let n = spec.component_count();
        let (inv, _) = cell_inventory(&spec, &rock, 0, &c).unwrap();
        let acc = cell_accumulation(
            &spec,
            &rock,
            0,
            &c,
            &inv.component_moles,
            &vec![0.0; n],
            1.0,
        )
        .unwrap();

        for i in 0..n {
            assert_eq!(
                acc.residual[i], 0.0,
                "{name}: component {i} has a nonzero residual at an unchanged state"
            );
        }
    }
}

/// An imposed source is accounted exactly: the residual moves by `-dt * source` and by nothing
/// else. This is what makes a well's contribution auditable later.
#[test]
fn comp_accumulation_an_imposed_source_is_accounted_exactly() {
    let rock = rock();
    let spec = pinned::ternary().unwrap();
    let c = cell(150.0, &[0.2, 0.5]);
    let (inv, _) = cell_inventory(&spec, &rock, 0, &c).unwrap();

    let dt = 2.5;
    let source = [10.0, -25.0, 0.0];
    let acc = cell_accumulation(&spec, &rock, 0, &c, &inv.component_moles, &source, dt).unwrap();

    for i in 0..3 {
        assert!(
            (acc.residual[i] - (-dt * source[i])).abs() < 1e-9,
            "component {i}: residual {} for source {} over {dt} days",
            acc.residual[i],
            source[i]
        );
    }
}

/// A depleted cell's residual must equal the inventory it lost. Checked against an inventory taken
/// at a different pressure rather than against the implementation.
#[test]
fn comp_accumulation_residual_equals_the_inventory_change() {
    let rock = rock();
    let spec = pinned::binary().unwrap();

    let previous = cell(200.0, &[0.6]);
    let current = cell(180.0, &[0.6]);
    let (prev_inv, _) = cell_inventory(&spec, &rock, 0, &previous).unwrap();
    let (curr_inv, _) = cell_inventory(&spec, &rock, 0, &current).unwrap();

    let acc = cell_accumulation(
        &spec,
        &rock,
        0,
        &current,
        &prev_inv.component_moles,
        &[0.0, 0.0],
        1.0,
    )
    .unwrap();

    for i in 0..2 {
        let expected = curr_inv.component_moles[i] - prev_inv.component_moles[i];
        assert!(rel(acc.residual[i], expected) < 1e-12, "component {i}");
        assert!(
            acc.residual[i] < 0.0,
            "component {i} should have lost moles on depletion"
        );
    }
}

#[test]
fn comp_accumulation_rejects_invalid_arguments() {
    let rock = rock();
    let spec = pinned::binary().unwrap();
    let c = cell(150.0, &[0.6]);

    for dt in [0.0, -1.0, f64::NAN] {
        assert!(matches!(
            cell_accumulation(&spec, &rock, 0, &c, &[1.0, 1.0], &[0.0, 0.0], dt),
            Err(AccumulationError::NonPositiveTimestep { .. })
        ));
    }
    assert!(matches!(
        cell_accumulation(&spec, &rock, 0, &c, &[1.0], &[0.0, 0.0], 1.0),
        Err(AccumulationError::InvalidSource { .. })
    ));
    assert!(matches!(
        cell_accumulation(&spec, &rock, 0, &c, &[1.0, -1.0], &[0.0, 0.0], 1.0),
        Err(AccumulationError::InvalidSource { .. })
    ));
    assert!(matches!(
        cell_accumulation(&spec, &rock, 0, &c, &[1.0, 1.0], &[0.0, f64::NAN], 1.0),
        Err(AccumulationError::InvalidSource { .. })
    ));
}

// ---------------------------------------------------------------------------------------------
// Jacobian
// ---------------------------------------------------------------------------------------------

/// Every Jacobian entry against a central difference, in **both** phase regimes. The FD
/// perturbation of a composition coordinate moves the dependent component the other way, so the
/// state stays on the simplex and the derivative measured is the one the layout defines.
#[test]
fn comp_accumulation_jacobian_matches_finite_differences_in_both_regimes() {
    let rock = rock();
    let mut worst = (0.0f64, String::new());

    for (name, spec, c) in regimes() {
        let n = spec.component_count();
        let previous = vec![0.0; n];
        let source = vec![0.0; n];
        let acc = cell_accumulation(&spec, &rock, 0, &c, &previous, &source, 1.0).unwrap();

        // Pressure column, in bar.
        let h_p = 1e-3;
        let up = cell_accumulation(
            &spec,
            &rock,
            0,
            &cell(c.pressure_bar + h_p, c.independent_z()),
            &previous,
            &source,
            1.0,
        )
        .unwrap();
        let down = cell_accumulation(
            &spec,
            &rock,
            0,
            &cell(c.pressure_bar - h_p, c.independent_z()),
            &previous,
            &source,
            1.0,
        )
        .unwrap();
        for i in 0..n {
            let fd = (up.residual[i] - down.residual[i]) / (2.0 * h_p);
            let e = (acc.jacobian[i][0] - fd).abs() / fd.abs().max(1.0);
            if e > worst.0 {
                worst = (e, format!("{name}/dR{i}/dp"));
            }
        }

        // Composition columns.
        let h_z = 1e-6;
        for k in 0..(n - 1) {
            let mut zp = c.independent_z().to_vec();
            let mut zm = c.independent_z().to_vec();
            zp[k] += h_z;
            zm[k] -= h_z;
            let up = cell_accumulation(
                &spec,
                &rock,
                0,
                &cell(c.pressure_bar, &zp),
                &previous,
                &source,
                1.0,
            )
            .unwrap();
            let down = cell_accumulation(
                &spec,
                &rock,
                0,
                &cell(c.pressure_bar, &zm),
                &previous,
                &source,
                1.0,
            )
            .unwrap();
            for i in 0..n {
                let fd = (up.residual[i] - down.residual[i]) / (2.0 * h_z);
                let e = (acc.jacobian[i][1 + k] - fd).abs() / fd.abs().max(1.0);
                if e > worst.0 {
                    worst = (e, format!("{name}/dR{i}/dz{k}"));
                }
            }
        }
    }

    assert!(
        worst.0 < 1e-5,
        "the accumulation Jacobian disagrees with FD: worst {:e} at {}",
        worst.0,
        worst.1
    );
}

/// The Jacobian is `N x N` — as many rows as components, as many columns as primaries — and the
/// previous inventory and the source contribute nothing to it. They are constants with respect to
/// the current primaries, and a Jacobian that moved with them would be differentiating the wrong
/// function.
#[test]
fn comp_accumulation_jacobian_is_square_and_independent_of_the_previous_state() {
    let rock = rock();
    let spec = pinned::ternary().unwrap();
    let c = cell(150.0, &[0.2, 0.5]);

    let a = cell_accumulation(&spec, &rock, 0, &c, &[0.0; 3], &[0.0; 3], 1.0).unwrap();
    let b = cell_accumulation(
        &spec,
        &rock,
        0,
        &c,
        &[1e5, 2e5, 3e5],
        &[10.0, -20.0, 5.0],
        7.0,
    )
    .unwrap();

    assert_eq!(a.jacobian.len(), 3);
    assert_eq!(a.jacobian[0].len(), 3);
    assert_eq!(a.jacobian, b.jacobian);
    assert_ne!(a.residual, b.residual);
}

/// The pressure column must be per bar, not per pascal. A factor of 1e5 in a Jacobian column is
/// the kind of error that makes a Newton step look like a line search failure.
#[test]
fn comp_accumulation_pressure_column_is_per_bar() {
    let rock = rock();
    let spec = pinned::binary().unwrap();
    let c = cell(150.0, &[0.6]);
    let acc = cell_accumulation(&spec, &rock, 0, &c, &[0.0; 2], &[0.0; 2], 1.0).unwrap();

    // One bar of depletion removes a measurable but small fraction of the cell's moles. Per pascal
    // it would be five orders smaller, which is the check.
    let total: f64 = acc.inventory.component_moles.iter().sum();
    let d_total: f64 = (0..2).map(|i| acc.jacobian[i][0]).sum();
    let fractional = d_total / total;
    assert!(
        fractional > 1e-5 && fractional < 1e-1,
        "d(total moles)/dp = {fractional} per unit of the pressure primary; \
         that is not a per-bar derivative"
    );
    // And it is positive: a cell at higher pressure holds more moles.
    assert!(d_total > 0.0);
}

/// Permuting the components permutes the residual and the Jacobian, and changes nothing else. The
/// model must not depend on which component was written first.
#[test]
fn comp_accumulation_is_equivariant_under_component_permutation() {
    let rock = rock();
    let spec = pinned::ternary().unwrap();
    let z = [0.2, 0.5, 0.3];

    let base = cell_accumulation(
        &spec,
        &rock,
        0,
        &cell(150.0, &z[..2]),
        &[0.0; 3],
        &[0.0; 3],
        1.0,
    )
    .unwrap();

    // Swap the first two components. The dependent component is unchanged, so the independent
    // coordinates simply swap.
    let order = [1, 0, 2];
    let permuted_spec = spec.permuted(&order).unwrap();
    let permuted_z = [z[1], z[0]];
    let permuted = cell_accumulation(
        &permuted_spec,
        &rock,
        0,
        &cell(150.0, &permuted_z),
        &[0.0; 3],
        &[0.0; 3],
        1.0,
    )
    .unwrap();

    for (new_i, &old_i) in order.iter().enumerate() {
        assert!(
            rel(permuted.residual[new_i], base.residual[old_i]) < 1e-10,
            "residual row {new_i} does not match the original row {old_i}"
        );
    }
    assert!(
        rel(permuted.inventory.total_moles, base.inventory.total_moles) < 1e-12,
        "permuting components changed the total moles"
    );
}

/// The flash a cell's inventory is built on must be the flash at that cell's own primaries.
/// Evaluating the inventory against a phase split from elsewhere is the error this guards.
#[test]
fn comp_accumulation_uses_the_flash_at_its_own_primaries() {
    let rock = rock();
    let spec = pinned::ternary().unwrap();
    let c = cell(150.0, &[0.2, 0.5]);
    let (_, state) = cell_inventory(&spec, &rock, 0, &c).unwrap();

    let direct = flash(
        &spec,
        bar_to_pa(c.pressure_bar),
        spec.reservoir_temperature_k(),
        &c.overall_composition(),
        None,
    )
    .unwrap();
    assert_eq!(state, direct);
}

// ---------------------------------------------------------------------------------------------
// Scaling
// ---------------------------------------------------------------------------------------------

/// A residual scaled by its own component's inventory is a relative error, and a residual that is
/// a fixed fraction of every component's inventory must scale to the same number in every row.
#[test]
fn comp_scaling_makes_rows_comparable() {
    let previous = [1.0e6, 1.0e3, 5.0e5];
    let scaling = EquationScaling::from_previous_inventory(&previous, 150.0);

    // One part in 1e4 of each component.
    let residual: Vec<f64> = previous.iter().map(|m| m * 1e-4).collect();
    let scaled = scaling.scale_residual(&residual);
    for (i, s) in scaled.iter().enumerate() {
        assert!((s - 1e-4).abs() < 1e-15, "row {i} scaled to {s}");
    }
    assert!((scaling.scaled_residual_norm(&residual) - 1e-4).abs() < 1e-15);
}

/// The failure this scaling exists to prevent: a trace component stalling unnoticed behind a large
/// one. Under a single cell-wide scale the trace row's own error is invisible; under per-component
/// scaling it dominates the norm, which is what should happen.
#[test]
fn comp_scaling_does_not_let_a_large_component_hide_a_trace_one() {
    let previous = [1.0e6, 1.0e2, 1.0e6];
    let scaling = EquationScaling::from_previous_inventory(&previous, 150.0);

    // The big components are converged to 1e-10 of themselves; the trace one is out by 20%.
    let residual = [1.0e-4, 2.0e1, 1.0e-4];

    let cell_wide: f64 = residual
        .iter()
        .map(|r| (r / previous.iter().sum::<f64>()).abs())
        .fold(0.0, f64::max);
    assert!(
        cell_wide < 1e-5,
        "a cell-wide scale reports {cell_wide:e}, which would look converged"
    );

    let norm = scaling.scaled_residual_norm(&residual);
    assert!(
        norm > 0.1,
        "per-component scaling reports {norm:e}; the trace component should dominate"
    );
}

/// An absent or trace component gets the floor rather than its own inventory, so nothing divides
/// by zero and a component holding almost nothing is not held to an impossible relative standard.
#[test]
fn comp_scaling_floors_absent_and_trace_components() {
    // Component 0 holds the cell; 1 is absent; 2 is below the floor; 3 is small but above it.
    // The last pair is the discriminating case — the floor is a fraction of the cell, not an
    // absolute mole count, so "small" and "below the floor" are different things.
    let previous = [1.0e6, 0.0, 1.0e-4, 1.0];
    let total: f64 = previous.iter().sum();
    let floor = TRACE_COMPONENT_FLOOR * total;
    assert!(
        previous[2] < floor && previous[3] > floor,
        "the fixture no longer straddles the floor: floor is {floor}"
    );

    let scaling = EquationScaling::from_previous_inventory(&previous, 150.0);
    assert_eq!(scaling.diagnostics.floored, vec![false, true, true, false]);
    assert!(scaling.row_scale.iter().all(|s| s.is_finite() && *s > 0.0));

    assert!((scaling.row_scale[1] - floor).abs() < 1e-9 * floor);
    assert!((scaling.row_scale[2] - floor).abs() < 1e-9 * floor);
    assert_eq!(scaling.row_scale[3], previous[3]);

    // A residual in the absent component is finite and visible, not infinite and not zero.
    let scaled = scaling.scale_residual(&[0.0, 1.0, 0.0, 0.0]);
    assert!(scaled[1].is_finite() && scaled[1] > 0.0);
}

/// An entirely empty cell must not produce a NaN scale. Degenerate, but reachable — a cell can be
/// initialized before it has any fluid in it.
#[test]
fn comp_scaling_survives_an_empty_cell() {
    let scaling = EquationScaling::from_previous_inventory(&[0.0, 0.0], 150.0);
    assert!(scaling.row_scale.iter().all(|s| s.is_finite() && *s > 0.0));
    assert!(scaling.scaled_residual_norm(&[0.0, 0.0]).is_finite());
    assert_eq!(scaling.diagnostics.reference_total_moles, 0.0);
}

/// The scaling is built from the **previous** inventory. If it moved with the trial state, a step
/// could report convergence by shrinking its own denominator.
#[test]
fn comp_scaling_is_referenced_to_the_previous_state() {
    let previous = [1.0e6, 1.0e6];
    let scaling = EquationScaling::from_previous_inventory(&previous, 150.0);
    assert_eq!(scaling.row_scale, previous.to_vec());
    assert_eq!(scaling.diagnostics.reference_total_moles, 2.0e6);
    assert_eq!(scaling.diagnostics.min_row_scale, 1.0e6);
    assert_eq!(scaling.diagnostics.max_row_scale, 1.0e6);
}

/// Primary-variable scaling: pressure against a declared reference, compositions against one.
#[test]
fn comp_scaling_primary_variables_use_declared_references() {
    let scaling = EquationScaling::from_previous_inventory(&[1.0, 1.0, 1.0], 250.0);
    assert_eq!(scaling.primary_scale.len(), 3);
    assert_eq!(scaling.primary_scale[0], 250.0);
    assert_eq!(scaling.primary_scale[1], 1.0);
    assert_eq!(scaling.primary_scale[2], 1.0);
}

/// Scaling a stationary cell's residual gives exactly zero, and the whole chain from state to
/// scaled norm holds together.
#[test]
fn comp_scaling_reports_zero_for_a_stationary_cell() {
    let rock = rock();
    for (name, spec, c) in regimes() {
        let n = spec.component_count();
        let (inv, _) = cell_inventory(&spec, &rock, 0, &c).unwrap();
        let acc = cell_accumulation(
            &spec,
            &rock,
            0,
            &c,
            &inv.component_moles,
            &vec![0.0; n],
            1.0,
        )
        .unwrap();
        let scaling =
            EquationScaling::from_previous_inventory(&inv.component_moles, c.pressure_bar);
        assert_eq!(
            scaling.scaled_residual_norm(&acc.residual),
            0.0,
            "{name}: a stationary cell has a nonzero scaled norm"
        );
    }
}
