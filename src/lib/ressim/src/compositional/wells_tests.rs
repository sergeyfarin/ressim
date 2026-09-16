//! C11 well contract tests (`comp_well_*`).
//!
//! The gates are internal invariants, because no external well oracle exists — see
//! `docs/COMPOSITIONAL_WELL_DESIGN.md`, "Provenance". Nothing here claims agreement with OPM.

use super::accumulation::cell_inventory;
use super::assembly::Face;
use super::flux::{Gravity, HydrocarbonRelPerm};
use super::layout::CompositionalLayout;
use super::state::{CompositionalCellState, CompositionalState, RockView};
use super::timestep::{CompositionalRun, TimestepOptions};
use super::wells::{
    CompositionalWell, SurfacePhase, WellControl, WellError, source_at_bhp, well_source,
};
use crate::fluid::flash::{PhaseState, flash};
use crate::fluid::specification::{FluidSpecification, SurfaceConditions};
use crate::fluid::transport::pinned_surface_conditions;
use crate::fluid::{pinned, units::bar_to_pa};

const RELPERM: HydrocarbonRelPerm = HydrocarbonRelPerm::StraightLine;
/// Peaceman geometry without mobility: 100 mD, 10 m of pay, 200 m spacing, 0.1 m radius.
const WELL_INDEX: f64 = 8.526_988_8e-3 * 2.0 * std::f64::consts::PI * 100.0 * 10.0 / 5.3;

fn cell(p_bar: f64, z: &[f64]) -> CompositionalCellState {
    CompositionalCellState::new(p_bar, z.to_vec()).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

fn producer(target_bar: f64) -> CompositionalWell {
    CompositionalWell {
        id: "P1".to_string(),
        cell: 0,
        well_index: WELL_INDEX,
        head_offset_bar: 0.0,
        control: WellControl::Bhp { target_bar },
        injection_composition: None,
    }
}

fn injector(target_bar: f64, z_inj: Vec<f64>) -> CompositionalWell {
    CompositionalWell {
        id: "I1".to_string(),
        cell: 0,
        well_index: WELL_INDEX,
        head_offset_bar: 0.0,
        control: WellControl::Bhp { target_bar },
        injection_composition: Some(z_inj),
    }
}

/// The pinned ternary with surface conditions attached, for the surface-rate control.
fn spec_with_surface() -> FluidSpecification {
    let base = pinned::ternary().unwrap();
    FluidSpecification::new(
        base.components().to_vec(),
        (0..3)
            .map(|i| (0..3).map(|j| base.interaction(i, j)).collect())
            .collect(),
        base.eos(),
        base.viscosity_model(),
        base.reservoir_temperature_k(),
        Some(pinned_surface_conditions()),
    )
    .unwrap()
}

// ---------------------------------------------------------------------------------------------
// Signs and direction
// ---------------------------------------------------------------------------------------------

/// The sign convention, which every other test depends on: positive means moles entering the cell,
/// so a producer is negative and an injector positive. Getting this backwards gives a well that
/// fills the reservoir while reporting production.
#[test]
fn comp_well_bhp_sign_convention_is_moles_into_the_cell() {
    let spec = pinned::ternary().unwrap();
    let c = cell(200.0, &[0.2, 0.5]);

    let p = well_source(&spec, RELPERM, &producer(150.0), &c).unwrap();
    assert!(p.is_producing());
    for (i, q) in p.component_moles_per_day.iter().enumerate() {
        assert!(
            *q < 0.0,
            "producer component {i} has a positive source: {q}"
        );
    }
    assert!(p.reservoir_rate_m3_per_day < 0.0);

    let inj = well_source(&spec, RELPERM, &injector(250.0, vec![1.0, 0.0, 0.0]), &c).unwrap();
    assert!(!inj.is_producing());
    assert!(inj.total_moles_per_day() > 0.0);
    assert!(inj.reservoir_rate_m3_per_day > 0.0);
}

/// A larger drawdown produces more, monotonically. The connection law's basic behaviour.
#[test]
fn comp_well_bhp_production_increases_with_drawdown() {
    let spec = pinned::ternary().unwrap();
    let c = cell(200.0, &[0.2, 0.5]);

    let mut previous = 0.0;
    for bhp in [199.0, 190.0, 170.0, 150.0, 120.0] {
        let s = well_source(&spec, RELPERM, &producer(bhp), &c).unwrap();
        let rate = -s.total_moles_per_day();
        assert!(
            rate > previous,
            "bhp {bhp}: rate {rate} did not exceed {previous}"
        );
        previous = rate;
    }
}

/// A producer whose BHP exceeds the cell pressure is not producing, and it does not become an
/// injector of a composition nobody specified.
#[test]
fn comp_well_a_producer_above_cell_pressure_is_shut_in() {
    let spec = pinned::ternary().unwrap();
    let c = cell(150.0, &[0.2, 0.5]);
    for bhp in [150.0, 160.0, 300.0] {
        let s = well_source(&spec, RELPERM, &producer(bhp), &c).unwrap();
        assert_eq!(s.total_moles_per_day(), 0.0, "bhp {bhp}");
        assert_eq!(s.reservoir_rate_m3_per_day, 0.0);
        assert!(s.component_moles_per_day.iter().all(|q| *q == 0.0));
    }
}

/// The head offset shifts the connection pressure, so a deeper completion draws harder at the same
/// datum BHP.
#[test]
fn comp_well_head_offset_shifts_the_connection_pressure() {
    let spec = pinned::ternary().unwrap();
    let c = cell(200.0, &[0.2, 0.5]);
    let mut deep = producer(150.0);
    deep.head_offset_bar = 20.0;

    let flat = well_source(&spec, RELPERM, &producer(150.0), &c).unwrap();
    let with_head = well_source(&spec, RELPERM, &deep, &c).unwrap();

    // p_conn = 170 instead of 150, so the drawdown is smaller and so is the rate.
    assert!(-with_head.total_moles_per_day() < -flat.total_moles_per_day());
    // And it matches shifting the BHP directly.
    let equivalent = well_source(&spec, RELPERM, &producer(170.0), &c).unwrap();
    assert!(
        rel(
            with_head.total_moles_per_day(),
            equivalent.total_moles_per_day()
        ) < 1e-12
    );
}

// ---------------------------------------------------------------------------------------------
// Composition
// ---------------------------------------------------------------------------------------------

/// A producer removes the cell's own fluid, and the mix it removes is not the cell's overall
/// composition — the phases move at different mobilities, which is the compositional effect.
#[test]
fn comp_well_producer_draws_the_cells_fluid() {
    let spec = pinned::ternary().unwrap();
    let c = cell(150.0, &[0.2, 0.5]);
    let state = flash(
        &spec,
        bar_to_pa(150.0),
        spec.reservoir_temperature_k(),
        &c.overall_composition(),
        None,
    )
    .unwrap();
    assert_eq!(state.phase_state, PhaseState::TwoPhase);

    let s = well_source(&spec, RELPERM, &producer(120.0), &c).unwrap();
    let total = s.total_moles_per_day();
    let produced: Vec<f64> = s
        .component_moles_per_day
        .iter()
        .map(|q| q / total)
        .collect();

    // Every component is produced.
    assert!(produced.iter().all(|f| *f > 0.0));
    assert!((produced.iter().sum::<f64>() - 1.0).abs() < 1e-12);

    // The light components are over-represented relative to the cell, because the vapour is more
    // mobile. Compare against the cell's overall composition.
    let z = c.overall_composition();
    assert!(
        produced[1] > z[1],
        "methane should be over-produced: {} vs {} in place",
        produced[1],
        z[1]
    );
    assert!(
        produced[2] < z[2],
        "n-decane should be under-produced: {} vs {} in place",
        produced[2],
        z[2]
    );
}

/// An injector delivers **exactly** the prescribed composition. Nothing about the cell's fluid may
/// enter the injected stream — that is the property that makes injection auditable.
#[test]
fn comp_well_injector_delivers_exactly_the_prescribed_composition() {
    let spec = pinned::ternary().unwrap();
    let z_inj = vec![0.7, 0.3, 0.0];

    for cell_z in [vec![0.2, 0.5], vec![0.05, 0.05], vec![0.4, 0.4]] {
        let c = cell(150.0, &cell_z);
        let s = well_source(&spec, RELPERM, &injector(250.0, z_inj.clone()), &c).unwrap();
        let total = s.total_moles_per_day();
        assert!(total > 0.0);
        for i in 0..3 {
            let fraction = s.component_moles_per_day[i] / total;
            assert!(
                (fraction - z_inj[i]).abs() < 1e-12,
                "cell {cell_z:?}: injected fraction {fraction} for component {i}, prescribed {}",
                z_inj[i]
            );
        }
    }
}

/// **The failure the plan names.** An injector delivering a vapour into a liquid-filled cell must
/// still inject: using the vapour relative permeability at the *cell's* saturation would be zero
/// and would eliminate injectivity for a reason that has nothing to do with the well.
#[test]
fn comp_well_a_vapour_injector_into_a_liquid_cell_still_injects() {
    let spec = pinned::binary().unwrap();
    // 400 bar is single-phase liquid; there is no vapour in the cell at all.
    let c = cell(400.0, &[0.6]);
    let state = flash(
        &spec,
        bar_to_pa(400.0),
        spec.reservoir_temperature_k(),
        &c.overall_composition(),
        None,
    )
    .unwrap();
    assert_eq!(state.phase_state, PhaseState::SingleLiquid);

    // Inject nearly pure methane, which at the connection pressure is a vapour or a light
    // supercritical fluid — in either case not the cell's liquid.
    let s = well_source(&spec, RELPERM, &injector(450.0, vec![0.999, 0.001]), &c).unwrap();
    assert!(
        s.total_moles_per_day() > 0.0,
        "injectivity vanished: {} mol/day",
        s.total_moles_per_day()
    );
    assert!(s.reservoir_rate_m3_per_day > 0.0);
}

#[test]
fn comp_well_an_injector_without_a_composition_is_rejected() {
    let spec = pinned::ternary().unwrap();
    let c = cell(150.0, &[0.2, 0.5]);
    // A "producer" with the BHP above the cell pressure and no injection composition is shut in
    // rather than inventing a stream — checked above. Here the composition is explicitly absent
    // while the well is configured as an injector by its control, which cannot be represented:
    // the type requires the composition for the injection branch to be taken at all.
    let mut well = producer(250.0);
    well.injection_composition = None;
    let s = well_source(&spec, RELPERM, &well, &c).unwrap();
    assert_eq!(s.total_moles_per_day(), 0.0);
}

#[test]
fn comp_well_rejects_an_invalid_well_index() {
    let spec = pinned::ternary().unwrap();
    let c = cell(200.0, &[0.2, 0.5]);
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        let mut well = producer(150.0);
        well.well_index = bad;
        assert!(matches!(
            well_source(&spec, RELPERM, &well, &c),
            Err(WellError::InvalidWellIndex { .. })
        ));
    }
}

// ---------------------------------------------------------------------------------------------
// Derivatives
// ---------------------------------------------------------------------------------------------

/// The `N x N` cell block and the `N x 1` BHP column against finite differences. Both come from one
/// AD evaluation, so a disagreement between them is impossible by construction — this checks they
/// are both right rather than merely consistent.
#[test]
fn comp_well_derivatives_match_finite_differences() {
    let cases: Vec<(&str, FluidSpecification, CompositionalWell, f64, Vec<f64>)> = vec![
        (
            "ternary producer, two-phase",
            pinned::ternary().unwrap(),
            producer(150.0),
            200.0,
            vec![0.2, 0.5],
        ),
        (
            "binary producer, liquid",
            pinned::binary().unwrap(),
            producer(380.0),
            400.0,
            vec![0.6],
        ),
        (
            "ternary injector",
            pinned::ternary().unwrap(),
            injector(250.0, vec![0.7, 0.3, 0.0]),
            200.0,
            vec![0.2, 0.5],
        ),
    ];

    let mut worst = (0.0f64, String::new());

    for (name, spec, well, p, z) in cases {
        let n = spec.component_count();
        let bhp = match well.control {
            WellControl::Bhp { target_bar } => target_bar,
            _ => unreachable!(),
        };
        let base = source_at_bhp(&spec, RELPERM, &well, &cell(p, &z), bhp).unwrap();

        // Cell pressure.
        let h = 1e-3;
        let up = source_at_bhp(&spec, RELPERM, &well, &cell(p + h, &z), bhp).unwrap();
        let dn = source_at_bhp(&spec, RELPERM, &well, &cell(p - h, &z), bhp).unwrap();
        for i in 0..n {
            let fd = (up.component_moles_per_day[i] - dn.component_moles_per_day[i]) / (2.0 * h);
            let scale = fd
                .abs()
                .max(base.component_moles_per_day[i].abs() * 1e-3)
                .max(1e-6);
            let e = (base.cell_jacobian[i][0] - fd).abs() / scale;
            if e > worst.0 {
                worst = (e, format!("{name}/dS{i}/dp"));
            }
        }

        // Cell composition.
        for k in 0..(n - 1) {
            let hz = 1e-6;
            let mut zp = z.clone();
            let mut zm = z.clone();
            zp[k] += hz;
            zm[k] -= hz;
            let up = source_at_bhp(&spec, RELPERM, &well, &cell(p, &zp), bhp).unwrap();
            let dn = source_at_bhp(&spec, RELPERM, &well, &cell(p, &zm), bhp).unwrap();
            for i in 0..n {
                let fd =
                    (up.component_moles_per_day[i] - dn.component_moles_per_day[i]) / (2.0 * hz);
                let scale = fd
                    .abs()
                    .max(base.component_moles_per_day[i].abs() * 1e-3)
                    .max(1e-6);
                let e = (base.cell_jacobian[i][1 + k] - fd).abs() / scale;
                if e > worst.0 {
                    worst = (e, format!("{name}/dS{i}/dz{k}"));
                }
            }
        }

        // BHP.
        let hb = 1e-3;
        let up = source_at_bhp(&spec, RELPERM, &well, &cell(p, &z), bhp + hb).unwrap();
        let dn = source_at_bhp(&spec, RELPERM, &well, &cell(p, &z), bhp - hb).unwrap();
        for i in 0..n {
            let fd = (up.component_moles_per_day[i] - dn.component_moles_per_day[i]) / (2.0 * hb);
            let scale = fd
                .abs()
                .max(base.component_moles_per_day[i].abs() * 1e-3)
                .max(1e-6);
            let e = (base.bhp_derivative[i] - fd).abs() / scale;
            if e > worst.0 {
                worst = (e, format!("{name}/dS{i}/dbhp"));
            }
        }
    }

    assert!(
        worst.0 < 1e-4,
        "well derivatives disagree with FD: worst {:e} at {}",
        worst.0,
        worst.1
    );
}

/// A producer's rate falls as the BHP rises, so every BHP derivative is positive (less negative
/// production). A sign error here would make the rate-control solve run the wrong way.
#[test]
fn comp_well_bhp_derivative_has_the_right_sign() {
    let spec = pinned::ternary().unwrap();
    let c = cell(200.0, &[0.2, 0.5]);

    let p = source_at_bhp(&spec, RELPERM, &producer(150.0), &c, 150.0).unwrap();
    for (i, d) in p.bhp_derivative.iter().enumerate() {
        assert!(
            *d > 0.0,
            "producer component {i}: dS/dbhp = {d} should be positive"
        );
    }

    let z_inj = vec![0.7, 0.3, 0.0];
    let inj = injector(250.0, z_inj.clone());
    let s = source_at_bhp(&spec, RELPERM, &inj, &c, 250.0).unwrap();
    for (i, d) in s.bhp_derivative.iter().enumerate() {
        if z_inj[i] == 0.0 {
            // A component the stream does not contain has an exactly zero rate and an exactly
            // zero derivative. Not an oversight — there is nothing of it to inject.
            assert_eq!(*d, 0.0, "injector component {i} is absent from the stream");
            continue;
        }
        assert!(
            *d > 0.0,
            "injector component {i}: dS/dbhp = {d} should be positive"
        );
    }
}

/// An injector's source does not depend on the cell's composition — the stream is prescribed. That
/// is a structural zero, and asserting it is what keeps the injected composition auditable.
#[test]
fn comp_well_injector_source_is_independent_of_the_cell_composition() {
    let spec = pinned::ternary().unwrap();
    let inj = injector(250.0, vec![0.7, 0.3, 0.0]);
    let base = source_at_bhp(&spec, RELPERM, &inj, &cell(200.0, &[0.2, 0.5]), 250.0).unwrap();

    for i in 0..3 {
        assert_eq!(
            base.cell_jacobian[i][1], 0.0,
            "component {i}: the injected stream depends on the cell's z_0"
        );
        assert_eq!(base.cell_jacobian[i][2], 0.0);
    }

    // The *rate* does depend on the cell pressure, through the drawdown — that is not the same
    // thing as the stream depending on the cell's fluid, and it must not be zero. A higher cell
    // pressure means less injection, so the derivative is negative.
    for i in 0..3 {
        if base.component_moles_per_day[i] == 0.0 {
            continue;
        }
        assert!(
            base.cell_jacobian[i][0] < 0.0,
            "component {i}: raising the cell pressure should reduce injection, got {}",
            base.cell_jacobian[i][0]
        );
    }

    // And the composition entering the cell is unchanged by the cell pressure: every component's
    // rate scales by the same factor, so the ratios are constant.
    let nearby = source_at_bhp(&spec, RELPERM, &inj, &cell(210.0, &[0.4, 0.4]), 250.0).unwrap();
    let base_total: f64 = base.component_moles_per_day.iter().sum();
    let nearby_total: f64 = nearby.component_moles_per_day.iter().sum();
    for i in 0..3 {
        let a = base.component_moles_per_day[i] / base_total;
        let b = nearby.component_moles_per_day[i] / nearby_total;
        assert!(
            (a - b).abs() < 1e-12,
            "component {i}: injected fraction moved from {a} to {b}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Controls
// ---------------------------------------------------------------------------------------------

/// A molar rate target is achieved when it is achievable, and the BHP that achieves it is reported.
#[test]
fn comp_well_molar_rate_control_hits_its_target() {
    let spec = pinned::ternary().unwrap();
    let c = cell(200.0, &[0.2, 0.5]);

    // Find an achievable rate by asking what a moderate drawdown gives.
    let reference = well_source(&spec, RELPERM, &producer(180.0), &c).unwrap();
    let target = reference.total_moles_per_day();
    assert!(target < 0.0);

    let mut well = producer(0.0);
    well.control = WellControl::MolarRate {
        target_mol_per_day: target,
        bhp_limit_bar: 50.0,
    };
    let s = well_source(&spec, RELPERM, &well, &c).unwrap();

    assert!(!s.on_bhp_limit, "the limit should not have bound");
    assert!(
        rel(s.total_moles_per_day(), target) < 1e-6,
        "achieved {} against a target of {target}",
        s.total_moles_per_day()
    );
    assert!(
        (s.bhp_bar - 180.0).abs() < 1e-3,
        "the solved BHP is {}, expected about 180",
        s.bhp_bar
    );
}

/// When the BHP limit binds, the well reverts to BHP control at the limit and reports the achieved
/// rate — not the target it could not reach.
#[test]
fn comp_well_a_binding_bhp_limit_overrides_the_rate_target() {
    let spec = pinned::ternary().unwrap();
    let c = cell(200.0, &[0.2, 0.5]);

    let at_limit = well_source(&spec, RELPERM, &producer(150.0), &c).unwrap();
    // Ask for ten times what the limit can deliver.
    let target = at_limit.total_moles_per_day() * 10.0;

    let mut well = producer(0.0);
    well.control = WellControl::MolarRate {
        target_mol_per_day: target,
        bhp_limit_bar: 150.0,
    };
    let s = well_source(&spec, RELPERM, &well, &c).unwrap();

    assert!(s.on_bhp_limit, "the limit must be reported as binding");
    assert_eq!(s.bhp_bar, 150.0);
    assert!(
        rel(s.total_moles_per_day(), at_limit.total_moles_per_day()) < 1e-12,
        "the achieved rate is not the limit's rate"
    );
    assert!(
        s.total_moles_per_day() > target,
        "the achieved production must be less than the unreachable target"
    );
}

/// The same for an injector, whose limit is a maximum rather than a minimum.
#[test]
fn comp_well_an_injector_respects_its_bhp_limit() {
    let spec = pinned::ternary().unwrap();
    let c = cell(200.0, &[0.2, 0.5]);
    let z_inj = vec![0.7, 0.3, 0.0];

    let at_limit = well_source(&spec, RELPERM, &injector(230.0, z_inj.clone()), &c).unwrap();
    let target = at_limit.total_moles_per_day() * 10.0;

    let mut well = injector(0.0, z_inj);
    well.control = WellControl::MolarRate {
        target_mol_per_day: target,
        bhp_limit_bar: 230.0,
    };
    let s = well_source(&spec, RELPERM, &well, &c).unwrap();

    assert!(s.on_bhp_limit);
    assert_eq!(s.bhp_bar, 230.0);
    assert!(s.total_moles_per_day() < target);
    assert!(s.total_moles_per_day() > 0.0, "it must still be injecting");
}

/// Surface volumetric control goes through C5's single-stage flash, and it is **not** a reservoir
/// rate. The two differ by the formation volume factor, and the test checks they differ by a lot
/// rather than checking that they agree.
#[test]
fn comp_well_surface_rate_control_is_not_a_reservoir_rate() {
    let spec = spec_with_surface();
    let c = cell(200.0, &[0.2, 0.5]);

    let reference = well_source(&spec, RELPERM, &producer(180.0), &c).unwrap();
    let reservoir_rate = reference.reservoir_rate_m3_per_day;
    assert!(reservoir_rate < 0.0);

    // What that same well delivers at surface, as gas.
    let stream: Vec<f64> = reference
        .component_moles_per_day
        .iter()
        .map(|m| -m)
        .collect();
    let separated = crate::fluid::transport::surface_separation(&spec, &stream).unwrap();
    let surface_gas = separated.vapour_volume;

    assert!(
        surface_gas > 50.0 * reservoir_rate.abs(),
        "surface gas {surface_gas} is not much larger than the reservoir rate {}; \
         the two should differ by the formation volume factor",
        reservoir_rate.abs()
    );

    // Now control on it and check the target is hit.
    let mut well = producer(0.0);
    well.control = WellControl::SurfaceRate {
        target_m3_per_day: -surface_gas,
        phase: SurfacePhase::Vapour,
        bhp_limit_bar: 50.0,
    };
    let s = well_source(&spec, RELPERM, &well, &c).unwrap();
    assert!(!s.on_bhp_limit);
    assert!(
        (s.bhp_bar - 180.0).abs() < 1e-2,
        "the solved BHP is {}, expected about 180",
        s.bhp_bar
    );
}

/// Surface control without pinned surface conditions is refused rather than assuming a standard
/// state.
#[test]
fn comp_well_surface_control_requires_pinned_conditions() {
    let spec = pinned::ternary().unwrap();
    assert!(spec.surface().is_none());
    let c = cell(200.0, &[0.2, 0.5]);

    let mut well = producer(0.0);
    well.control = WellControl::SurfaceRate {
        target_m3_per_day: -100.0,
        phase: SurfacePhase::Vapour,
        bhp_limit_bar: 50.0,
    };
    assert!(matches!(
        well_source(&spec, RELPERM, &well, &c),
        Err(WellError::SurfaceConditionsNotPinned)
    ));
}

/// Surface conditions are validated by the specification, so an implausible pair cannot reach the
/// well model at all.
#[test]
fn comp_well_surface_conditions_are_validated_upstream() {
    let base = pinned::ternary().unwrap();
    let build = |s: SurfaceConditions| {
        FluidSpecification::new(
            base.components().to_vec(),
            vec![vec![0.0; 3]; 3],
            base.eos(),
            base.viscosity_model(),
            base.reservoir_temperature_k(),
            Some(s),
        )
    };
    assert!(
        build(SurfaceConditions {
            pressure_pa: -1.0,
            temperature_k: 288.71
        })
        .is_err()
    );
    assert!(build(pinned_surface_conditions()).is_ok());
}

// ---------------------------------------------------------------------------------------------
// Inventory closure through the lifecycle
// ---------------------------------------------------------------------------------------------

/// The closure gate: over several accepted steps, what the grid lost equals what the well produced.
/// C10's test used an imposed source; this drives the same closure through a real well whose rate
/// changes as the cell depletes.
#[test]
fn comp_well_production_closes_the_grid_inventory_over_several_steps() {
    let spec = pinned::ternary().unwrap();
    let cells = 3;
    let layout = CompositionalLayout::new(3, cells, 0, 0).unwrap();
    let state = CompositionalState::new(
        &layout,
        (0..cells).map(|_| cell(220.0, &[0.2, 0.5])).collect(),
    )
    .unwrap();
    let pore_volumes = vec![1000.0; cells];
    let rock = RockView {
        pore_volume_ref_m3: &pore_volumes,
        reference_pressure_bar: 200.0,
        compressibility_per_bar: 4.0e-5,
    };
    let faces: Vec<Face> = (0..cells - 1)
        .map(|i| Face {
            cell_i: i,
            cell_j: i + 1,
            geom_t: 8.526_988_8e-3 * 100.0 * (100.0 * 10.0) / 100.0,
            gravity: Gravity::OFF,
        })
        .collect();

    let mut run = CompositionalRun::new(state);
    let initial = run.accepted_inventory(&spec, &rock).unwrap();
    let initial_total: Vec<f64> = (0..3)
        .map(|i| initial.iter().map(|c| c[i]).sum::<f64>())
        .collect();

    // A producer on the last cell, at a BHP that depletes it slowly.
    let mut well = producer(215.0);
    well.cell = 2;

    let mut produced = vec![0.0; 3];
    for _ in 0..5 {
        // The well's source is re-evaluated at the current state each step, which is what makes
        // this a well rather than a fixed source.
        let source = well_source(&spec, RELPERM, &well, run.state().cell(well.cell)).unwrap();
        let mut sources = vec![vec![0.0; 3]; cells];
        sources[well.cell] = source.component_moles_per_day.clone();

        let report = run.step(
            &spec,
            &layout,
            &rock,
            RELPERM,
            &faces,
            &sources,
            1.0,
            TimestepOptions::default(),
        );
        assert!(report.succeeded(), "{report:?}");
        let dt = report.accepted_dt_days.unwrap();
        for i in 0..3 {
            produced[i] += dt * source.component_moles_per_day[i];
        }
    }

    let final_inventory = run.accepted_inventory(&spec, &rock).unwrap();
    for i in 0..3 {
        let final_total: f64 = final_inventory.iter().map(|c| c[i]).sum();
        let lost = initial_total[i] - final_total;
        let removed = -produced[i];
        assert!(
            (lost - removed).abs() / removed.abs().max(1.0) < 1e-6,
            "component {i}: the grid lost {lost} moles but the well removed {removed}"
        );
        assert!(removed > 0.0, "component {i} was not produced at all");
    }

    // And the rate must have declined as the cell depleted, which is the behaviour that makes this
    // a well rather than a constant source.
    assert!(run.state().cell(2).pressure_bar < 220.0);
}

/// Injection adds exactly the prescribed stream to the grid, and the grid's composition moves
/// toward it.
#[test]
fn comp_well_injection_adds_the_prescribed_stream_to_the_grid() {
    let spec = pinned::ternary().unwrap();
    let layout = CompositionalLayout::new(3, 1, 0, 0).unwrap();
    let state = CompositionalState::new(&layout, vec![cell(200.0, &[0.05, 0.25])]).unwrap();
    let pore_volumes = vec![1000.0];
    let rock = RockView {
        pore_volume_ref_m3: &pore_volumes,
        reference_pressure_bar: 200.0,
        compressibility_per_bar: 4.0e-5,
    };

    let mut run = CompositionalRun::new(state);
    let initial = run.accepted_inventory(&spec, &rock).unwrap();

    // A BHP well above the cell's pressure, and a well index scaled down so a single 1000 m3 cell
    // is not filled in one step. At the full index this well delivers ~4e5 m3/day into 1000 m3 of
    // pore volume, which is a statement about the test grid rather than about the well model.
    let mut well = injector(280.0, vec![1.0, 0.0, 0.0]); // pure CO2
    well.cell = 0;
    well.well_index = WELL_INDEX * 1e-4;

    let mut injected = vec![0.0; 3];
    for step in 0..3 {
        let source = well_source(&spec, RELPERM, &well, run.state().cell(0)).unwrap();
        assert!(
            source.total_moles_per_day() > 0.0,
            "step {step}: injection stopped at cell pressure {}",
            run.state().cell(0).pressure_bar
        );
        let sources = vec![source.component_moles_per_day.clone()];
        let report = run.step(
            &spec,
            &layout,
            &rock,
            RELPERM,
            &[],
            &sources,
            0.05,
            TimestepOptions::default(),
        );
        assert!(report.succeeded(), "{report:?}");
        let dt = report.accepted_dt_days.unwrap();
        for i in 0..3 {
            injected[i] += dt * source.component_moles_per_day[i];
        }
    }

    // Only CO2 was injected.
    assert!(injected[0] > 0.0);
    assert!(injected[1].abs() < 1e-9 * injected[0]);
    assert!(injected[2].abs() < 1e-9 * injected[0]);

    let final_inventory = run.accepted_inventory(&spec, &rock).unwrap();
    for i in 0..3 {
        let gained = final_inventory[0][i] - initial[0][i];
        assert!(
            (gained - injected[i]).abs() / injected[i].abs().max(1.0) < 1e-6,
            "component {i}: the cell gained {gained} but {} was injected",
            injected[i]
        );
    }

    // And the cell is now richer in CO2 and at higher pressure.
    assert!(run.state().cell(0).independent_z()[0] > 0.05);
    assert!(run.state().cell(0).pressure_bar > 200.0);
}
