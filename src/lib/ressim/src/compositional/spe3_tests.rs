//! #51: fixed-pore-volume, gas-only 1D depletion through the dew point against JutulDarcy.
use super::{
    assembly::Face,
    flux::Gravity,
    layout::CompositionalLayout,
    relperm::{CoreyParameters, RelativePermeabilityModel},
    state::{CompositionalCellState, CompositionalState, RockView},
    timestep::{CompositionalRun, TimestepOptions},
    wells::{CompositionalWell, WellControl},
};
use crate::fluid::{
    flash::{PhaseState, flash},
    pinned,
    spe3_tests::reference,
};

#[test]
fn comp_spe3_depletion_crosses_dew_point_without_fragmentation_against_jutul() {
    let fixture = reference();
    let spec = pinned::spe3().unwrap();
    let layout = CompositionalLayout::new(9, 3, 1, 1).unwrap();
    let cells = (0..3)
        .map(|_| {
            CompositionalCellState::new(260.0, pinned::SPE3_COMPOSITION[..8].to_vec()).unwrap()
        })
        .collect();
    let mut run = CompositionalRun::new(CompositionalState::new(&layout, cells).unwrap());
    let rock = RockView {
        pore_volume_ref_m3: &[200.0; 3],
        reference_pressure_bar: 260.0,
        compressibility_per_bar: 0.0,
    };
    let initial = run.accepted_inventory(&spec, &rock).unwrap();
    let faces: Vec<_> = (0..2)
        .map(|i| Face {
            cell_i: i,
            cell_j: i + 1,
            geom_t: 8.5269888e-3 * 10.0 * 100.0 / 10.0,
            gravity: Gravity::OFF,
        })
        .collect();
    let wells = [CompositionalWell::single(
        "Producer",
        2,
        0.001,
        WellControl::Bhp { target_bar: 140.0 },
    )];
    let relperm = RelativePermeabilityModel::Corey(
        CoreyParameters::new(0.0, 0.0, 1.0, 1.0, 0.0, 1.0).unwrap(),
    );
    let sources = vec![vec![0.0; 9]; 3];
    let mut options = TimestepOptions::default();
    options.newton.tolerance = 1e-10;
    let mut worst_p = 0.0_f64;
    let mut worst_sl = 0.0_f64;
    let mut worst_z = 0.0_f64;
    let mut max_newton = 0;
    let mut two_phase = 0;
    assert_eq!(fixture.depletion.reports.len(), 150);
    for r in fixture.depletion.reports {
        let report = run.step(
            &spec, &layout, &rock, &relperm, &faces, &wells, &sources, 1.0, options,
        );
        assert_eq!(
            report.accepted_dt_days,
            Some(1.0),
            "day {}: {report:?}",
            r.day
        );
        assert_eq!(report.retries(), 0, "day {} fragmented", r.day);
        max_newton = max_newton.max(report.attempts[0].newton.iterations);
        for c in 0..3 {
            let cell = run.state().cell(c);
            let z = cell.overall_composition();
            let f = flash(
                &spec,
                cell.pressure_bar * 1e5,
                spec.reservoir_temperature_k(),
                &z,
                None,
            )
            .unwrap();
            let sl = f.liquid.as_ref().map_or(0.0, |p| {
                f.l_liquid() / p.molar_density / f.mixture_molar_volume()
            });
            two_phase += usize::from(f.phase_state == PhaseState::TwoPhase);
            worst_p = worst_p.max((cell.pressure_bar - r.p[c]).abs());
            worst_sl = worst_sl.max((sl - r.sl[c]).abs());
            for i in 0..9 {
                worst_z = worst_z.max((z[i] - r.z[c][i]).abs());
            }
        }
    }
    println!(
        "SPE3 depletion: max Δp {worst_p:e} bar, ΔSl {worst_sl:e}, Δz {worst_z:e}; max Newton {max_newton}; two-phase states {two_phase}/450"
    );
    assert!(
        two_phase > 100 && two_phase < 450,
        "the trajectory must cross its dew point"
    );
    // Same grid and backward-Euler report step. These bands measure simulator agreement,
    // not agreement of this verification case with the SPE3 laboratory CVD measurements.
    assert!(worst_p < 0.01, "pressure mismatch {worst_p:e} bar");
    assert!(worst_sl < 1e-4, "liquid saturation mismatch {worst_sl:e}");
    assert!(worst_z < 1e-5, "composition mismatch {worst_z:e}");
    let final_inventory = run.accepted_inventory(&spec, &rock).unwrap();
    for i in 0..9 {
        let before: f64 = initial.iter().map(|c| c[i]).sum();
        let after: f64 = final_inventory.iter().map(|c| c[i]).sum();
        assert!((after - before - run.cumulative_well_moles()[0][i]).abs() / before < 1e-8);
    }
}
