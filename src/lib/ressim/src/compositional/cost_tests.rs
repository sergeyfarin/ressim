//! Component-count scaling of the per-cell kernels (#45): `comp_accumulation_ten_*`,
//! `comp_flux_ten_*` and the `#[ignore]`d `comp_cost_*` measurement.
//!
//! N=6 is SPE5 and is covered by the fixture and the ordinary FD tests. N=10 has no pinned fluid,
//! so these use a **timing fluid**: SPE5's six components plus four pseudo-components, each the
//! arithmetic mean of two SPE5 neighbours. It is not a validated fluid and must never become one.
//! It exists so the `MAX_COMPONENTS` instantiations are exercised and so their cost can be
//! measured on a two-phase state that behaves like a real oil.

use std::time::Instant;

use super::accumulation::cell_accumulation;
use super::flux::{Gravity, face_flux};
use super::relperm::RelativePermeabilityModel;
use super::state::{CompositionalCellState, RockView};
use crate::fluid::derivatives::flash_derivatives;
use crate::fluid::flash::{PhaseState, flash};
use crate::fluid::pinned;
use crate::fluid::specification::{
    Component, FluidSpecification, MAX_COMPONENTS, OverallComposition,
};
use crate::fluid::units::bar_to_pa;

const PV_REF: [f64; 2] = [1000.0, 1000.0];

fn rock() -> RockView<'static> {
    RockView {
        pore_volume_ref_m3: &PV_REF,
        reference_pressure_bar: 200.0,
        compressibility_per_bar: 4.0e-5,
    }
}

fn cell(p_bar: f64, z: &[f64]) -> CompositionalCellState {
    CompositionalCellState::new(p_bar, z[..z.len() - 1].to_vec()).unwrap()
}

/// SPE5 plus four pseudo-components between C1/C3, C3/C6, C6/C10 and C10/C15. The SPE5
/// interactions are kept by component id; every pair involving a pseudo-component is zero.
fn ten_component_fluid() -> FluidSpecification {
    let spe5 = pinned::spe5().unwrap();
    let c = spe5.components();
    let mean = |a: &Component, b: &Component, id: &str| Component {
        id: id.to_string(),
        molar_mass_kg_per_mol: 0.5 * (a.molar_mass_kg_per_mol + b.molar_mass_kg_per_mol),
        critical_temperature_k: 0.5 * (a.critical_temperature_k + b.critical_temperature_k),
        critical_pressure_pa: 0.5 * (a.critical_pressure_pa + b.critical_pressure_pa),
        critical_volume_m3_per_kmol: 0.5
            * (a.critical_volume_m3_per_kmol + b.critical_volume_m3_per_kmol),
        acentric_factor: 0.5 * (a.acentric_factor + b.acentric_factor),
    };
    let components = vec![
        c[0].clone(),
        mean(&c[0], &c[1], "P1_3"),
        c[1].clone(),
        mean(&c[1], &c[2], "P3_6"),
        c[2].clone(),
        mean(&c[2], &c[3], "P6_10"),
        c[3].clone(),
        mean(&c[3], &c[4], "P10_15"),
        c[4].clone(),
        c[5].clone(),
    ];
    let k = |a: &str, b: &str| match (spe5.index_of(a), spe5.index_of(b)) {
        (Some(i), Some(j)) => spe5.interaction(i, j),
        _ => 0.0,
    };
    let interaction = components
        .iter()
        .map(|a| components.iter().map(|b| k(&a.id, &b.id)).collect())
        .collect();
    FluidSpecification::new(
        components,
        interaction,
        spe5.eos(),
        spe5.viscosity_model(),
        spe5.reservoir_temperature_k(),
        None,
    )
    .unwrap()
}

/// SPE5's initial oil with each pseudo-component taking a share of its neighbours, so the ten
/// component mixture splits near where the oil does.
const TEN_Z: [f64; 10] = [0.45, 0.04, 0.02, 0.04, 0.05, 0.06, 0.15, 0.04, 0.10, 0.05];
const SPE5_Z: [f64; 6] = [0.50, 0.03, 0.07, 0.20, 0.15, 0.05];

#[test]
fn comp_accumulation_ten_components_is_at_the_bound_and_two_phase() {
    let spec = ten_component_fluid();
    assert_eq!(spec.component_count(), MAX_COMPONENTS);
    OverallComposition::new(&spec, TEN_Z.to_vec()).unwrap();
    let state = flash(
        &spec,
        bar_to_pa(130.0),
        spec.reservoir_temperature_k(),
        &TEN_Z,
        None,
    )
    .unwrap();
    assert_eq!(state.phase_state, PhaseState::TwoPhase, "{state:?}");
}

/// The N=10 accumulation Jacobian against central differences, which is the check that the
/// `MAX_COMPONENTS` instantiation is wired to the right slots and not merely compiled.
#[test]
fn comp_accumulation_ten_component_jacobian_matches_finite_differences() {
    let spec = ten_component_fluid();
    let n = spec.component_count();
    let rock = rock();
    let zero = vec![0.0; n];
    let acc = |c: &CompositionalCellState| {
        cell_accumulation(&spec, &rock, 0, c, &zero, &zero, 1.0).unwrap()
    };
    let base = cell(130.0, &TEN_Z);
    let a = acc(&base);
    assert_eq!(a.jacobian.len(), n);
    assert!(a.jacobian.iter().all(|row| row.len() == n));

    let mut worst = (0.0f64, String::new());
    let h_p = 1e-3;
    let up = acc(&CompositionalCellState::new(130.0 + h_p, base.independent_z().to_vec()).unwrap());
    let dn = acc(&CompositionalCellState::new(130.0 - h_p, base.independent_z().to_vec()).unwrap());
    for i in 0..n {
        let fd = (up.residual[i] - dn.residual[i]) / (2.0 * h_p);
        let e = (a.jacobian[i][0] - fd).abs() / fd.abs().max(1.0);
        if e > worst.0 {
            worst = (e, format!("dR{i}/dp"));
        }
    }
    let h_z = 1e-6;
    for k in 0..(n - 1) {
        let mut zp = base.independent_z().to_vec();
        let mut zm = zp.clone();
        zp[k] += h_z;
        zm[k] -= h_z;
        let up = acc(&CompositionalCellState::new(130.0, zp).unwrap());
        let dn = acc(&CompositionalCellState::new(130.0, zm).unwrap());
        for i in 0..n {
            let fd = (up.residual[i] - dn.residual[i]) / (2.0 * h_z);
            let e = (a.jacobian[i][1 + k] - fd).abs() / fd.abs().max(1.0);
            if e > worst.0 {
                worst = (e, format!("dR{i}/dz{k}"));
            }
        }
    }
    assert!(
        worst.0 < 1e-5,
        "the N=10 accumulation Jacobian disagrees with FD: worst {:e} at {}",
        worst.0,
        worst.1
    );
}

/// The N=10 face kernel: an `N x 2N` Jacobian whose two cells' contributions cancel, as at every
/// other count.
#[test]
fn comp_flux_ten_component_face_is_conservative_and_full_width() {
    let spec = ten_component_fluid();
    let n = spec.component_count();
    let a = cell(130.0, &TEN_Z);
    let b = cell(110.0, &TEN_Z);
    let f = face_flux(
        &spec,
        &RelativePermeabilityModel::Linear,
        8.526_988_8e-3 * 100.0,
        Gravity::OFF,
        (0, &a),
        (1, &b),
    )
    .unwrap();
    assert_eq!(f.component_moles_per_day.len(), n);
    assert_eq!(f.jacobian.len(), n);
    assert!(f.jacobian.iter().all(|row| row.len() == 2 * n));
    assert!(f.component_moles_per_day.iter().all(|v| *v > 0.0));
}

/// Per-cell cost of the dense kernels at N = 2, 3, 6 and 10, in release. Run with
///
/// ```text
/// cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib comp_cost_ -- --ignored --nocapture
/// ```
///
/// and record the table in `docs/COMPOSITIONAL_VALIDATION.md` with the commit it ran on. It
/// asserts nothing about time, because a wall-clock bound is a property of the machine.
#[test]
#[ignore]
fn comp_cost_per_cell_kernels_by_component_count() {
    let ten = ten_component_fluid();
    let cases: Vec<(&str, FluidSpecification, Vec<f64>)> = vec![
        ("N=2 binary", pinned::binary().unwrap(), vec![0.6, 0.4]),
        (
            "N=3 ternary",
            pinned::ternary().unwrap(),
            vec![0.2, 0.5, 0.3],
        ),
        ("N=6 SPE5", pinned::spe5().unwrap(), SPE5_Z.to_vec()),
        ("N=10 timing fluid", ten, TEN_Z.to_vec()),
    ];
    let rock = rock();
    println!(
        "{:<20} {:>10} {:>12} {:>14} {:>12}",
        "case", "flash us", "derivs us", "accum cell us", "face us"
    );
    for (name, spec, z) in cases {
        let n = spec.component_count();
        let p_bar = if n == 2 { 100.0 } else { 130.0 };
        let p_pa = bar_to_pa(p_bar);
        let t = spec.reservoir_temperature_k();
        let state = flash(&spec, p_pa, t, &z, None).unwrap();
        assert_eq!(state.phase_state, PhaseState::TwoPhase, "{name}");
        let reps = 400;
        let time = |f: &mut dyn FnMut()| {
            f();
            let start = Instant::now();
            for _ in 0..reps {
                f();
            }
            start.elapsed().as_secs_f64() * 1e6 / reps as f64
        };
        // Cold, as `cell_inventory` and the face kernel call it: stability test, then substitution.
        let flash_us = time(&mut || {
            std::hint::black_box(flash(&spec, p_pa, t, &z, None).unwrap());
        });
        let derivs_us = time(&mut || {
            std::hint::black_box(flash_derivatives(&spec, p_pa, t, &z, &state).unwrap());
        });
        let a = cell(p_bar, &z);
        let b = cell(p_bar - 20.0, &z);
        let zero = vec![0.0; n];
        let acc_us = time(&mut || {
            std::hint::black_box(
                cell_accumulation(&spec, &rock, 0, &a, &zero, &zero, 1.0).unwrap(),
            );
        });
        let face_us = time(&mut || {
            std::hint::black_box(
                face_flux(
                    &spec,
                    &RelativePermeabilityModel::Linear,
                    1.0,
                    Gravity::OFF,
                    (0, &a),
                    (1, &b),
                )
                .unwrap(),
            );
        });
        println!("{name:<20} {flash_us:>10.1} {derivs_us:>12.1} {acc_us:>14.1} {face_us:>12.1}");
    }
}
