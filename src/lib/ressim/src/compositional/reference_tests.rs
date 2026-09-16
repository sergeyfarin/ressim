//! C12: the compositional model against an independent simulator (`comp_reference_*`).
//!
//! The reference is OPM's `flowexp_comp` running `OPM/opm-tests`'s `compositional/1D_COMP.DATA` —
//! a five-cell 1D CO2 flood in CO2 / methane / decane, which is ResSim's own V1 fluid. See
//! `docs/COMPOSITIONAL_VALIDATION.md` §3b for how the executable is built, and
//! `opm/compositional/1d_comp/` for the deck and the extracted fixture.
//!
//! # What this compares, and what it does not
//!
//! **This file compares thermodynamics along a trajectory, not the trajectory itself.** For every
//! cell at every report step, it takes the reference's own `(p, z)` and asks ResSim's flash what
//! the phase state and compositions are there, then compares against the reference's. That is 140
//! independent states spanning the whole displacement, from the initial two-phase mixture to
//! essentially pure CO2 after breakthrough.
//!
//! It is a genuinely external check: the fluid data was entered separately in the deck, the EOS
//! and flash are a different implementation in a different language, and nothing in ResSim was
//! tuned against it.
//!
//! Reproducing the **transport** — running ResSim's own timestepping and comparing the resulting
//! trajectory — is the other half of C12 and is not done here. Doing so needs the deck's
//! transmissibility, well model and timestep control matched as well, and each of those is a
//! separate place for two simulators to differ; conflating them with a thermodynamic comparison
//! would make a disagreement uninterpretable.
//!
//! # The deck's fluid is not the pinned fluid
//!
//! Close, but not identical — the deck carries more precise critical properties than OPM's
//! hard-coded `ThreeComponentFluidSystem`. The specification used here is built from **the deck's**
//! numbers, because the deck is what the reference ran. Using the pinned ones would compare two
//! different problems.

use super::layout::CompositionalLayout;
use super::state::{CompositionalCellState, CompositionalState};
use crate::fluid::flash::{PhaseState, flash};
use crate::fluid::specification::{
    Component, EosVariant, FluidSpecification, SurfaceConditions, ViscosityModel,
};
use crate::fluid::transport::flash_viscosities;
use crate::fluid::units::bar_to_pa;
use serde::Deserialize;

/// The reference fixture, as extracted by `tools/opm_compositional/extract_reference.py`.
#[derive(Debug, Deserialize)]
struct Reference {
    schema: String,
    case: String,
    cells: usize,
    report_steps: Vec<ReferenceStep>,
    summary: std::collections::BTreeMap<String, Vec<f64>>,
}

#[derive(Debug, Deserialize)]
struct ReferenceStep {
    sequence: i64,
    pressure: Vec<f64>,
    sgas: Vec<f64>,
    soil: Vec<f64>,
    xmf1: Vec<f64>,
    xmf2: Vec<f64>,
    xmf3: Vec<f64>,
    ymf1: Vec<f64>,
    ymf2: Vec<f64>,
    ymf3: Vec<f64>,
    zmf1: Vec<f64>,
    zmf2: Vec<f64>,
    zmf3: Vec<f64>,
}

impl ReferenceStep {
    fn z(&self, cell: usize) -> [f64; 3] {
        [self.zmf1[cell], self.zmf2[cell], self.zmf3[cell]]
    }
    fn x(&self, cell: usize) -> [f64; 3] {
        [self.xmf1[cell], self.xmf2[cell], self.xmf3[cell]]
    }
    fn y(&self, cell: usize) -> [f64; 3] {
        [self.ymf1[cell], self.ymf2[cell], self.ymf3[cell]]
    }
}

fn reference() -> Reference {
    let raw = include_str!("../../../../../opm/compositional/1d_comp/reference.json");
    let r: Reference = serde_json::from_str(raw).expect("the reference fixture must parse");
    assert_eq!(r.schema, "ressim-compositional-reference/1");
    assert_eq!(r.case, "1D_COMP");
    assert_eq!(r.cells, 5);
    assert_eq!(r.report_steps.len(), 28);
    r
}

/// The deck's fluid, transcribed from `1D_COMP.DATA`'s PROPS section.
///
/// Every value is the deck's, including the ones that differ from OPM's hard-coded
/// `ThreeComponentFluidSystem`. `PCRIT` is in bar in the deck (METRIC) and `MW` in g/mol;
/// both are converted here, at the boundary, exactly once.
fn deck_fluid() -> FluidSpecification {
    let component =
        |id: &str, mw_g_per_mol: f64, tc_k: f64, pc_bar: f64, vc: f64, acf: f64| Component {
            id: id.to_string(),
            molar_mass_kg_per_mol: mw_g_per_mol * 1e-3,
            critical_temperature_k: tc_k,
            critical_pressure_pa: pc_bar * 1e5,
            critical_volume_m3_per_kmol: vc,
            acentric_factor: acf,
        };
    FluidSpecification::new(
        vec![
            component("CO2", 44.00, 304.128, 73.773, 0.09412, 0.22394),
            component("METHANE", 16.04, 190.564, 45.992, 0.09863, 0.01142),
            component("DECANE", 142.28, 617.7, 21.03, 0.60980, 0.4884),
        ],
        // BIC: all zero in the deck.
        vec![vec![0.0; 3]; 3],
        // EOS: PR.
        EosVariant::PengRobinson,
        ViscosityModel::LohrenzBrayClark,
        // TEMPI 150 (deg C).
        crate::fluid::units::from_celsius(150.0),
        // STCOND 15.0 1.0 — the deck's own surface conditions, not the pinned ones.
        Some(SurfaceConditions {
            pressure_pa: 1.0e5,
            temperature_k: crate::fluid::units::from_celsius(15.0),
        }),
    )
    .expect("the deck's fluid must validate")
}

/// The deck's fluid differs from the pinned one, and the difference is recorded rather than
/// smoothed over. If these ever coincide, the pinned data was changed and the comparison below
/// stops being independent.
#[test]
fn comp_reference_deck_fluid_differs_from_the_pinned_fluid() {
    let deck = deck_fluid();
    let pinned = crate::fluid::pinned::ternary().unwrap();

    assert_eq!(deck.component_count(), pinned.component_count());
    let mut differences = 0;
    for i in 0..3 {
        let d = deck.component(i);
        let p = pinned.component(i);
        // Critical volumes and the interaction matrix DO match; the rest is more precise here.
        assert!(
            (d.critical_volume_m3_per_kmol - p.critical_volume_m3_per_kmol).abs() < 1e-12,
            "component {i}: the critical volumes were expected to match"
        );
        for (a, b) in [
            (d.acentric_factor, p.acentric_factor),
            (d.critical_temperature_k, p.critical_temperature_k),
            (d.critical_pressure_pa, p.critical_pressure_pa),
            (d.molar_mass_kg_per_mol, p.molar_mass_kg_per_mol),
        ] {
            if (a - b).abs() > 1e-12 * b.abs().max(1.0) {
                differences += 1;
            }
        }
    }
    assert!(
        differences >= 6,
        "the deck's fluid now matches the pinned one in {differences} places; the comparison is \
         no longer against independently entered data"
    );

    // The deck also carries its own surface conditions, which are not the pinned ones.
    let surface = deck.surface().unwrap();
    assert_eq!(surface.pressure_pa, 1.0e5);
    assert!((surface.temperature_k - 288.15).abs() < 1e-9);
    assert_ne!(
        surface.pressure_pa,
        crate::fluid::transport::STANDARD_PRESSURE_PA
    );
}

/// **The headline C12 result.** At every cell of every report step, flash the reference's own
/// `(p, z)` and compare the phase state, the saturation and both phase compositions.
///
/// 140 states, spanning the initial two-phase mixture through CO2 breakthrough to essentially pure
/// CO2. The reference writes single precision, so agreement is bounded by its output resolution
/// rather than by either implementation.
#[test]
fn comp_reference_flash_matches_opm_across_the_whole_trajectory() {
    let reference = reference();
    let spec = deck_fluid();
    let t = spec.reservoir_temperature_k();

    let mut worst_saturation = (0.0f64, String::new());
    let mut worst_composition = (0.0f64, String::new());
    let mut two_phase = 0usize;
    let mut single_phase = 0usize;
    let mut compared = 0usize;

    for step in &reference.report_steps {
        for cell in 0..reference.cells {
            let p = step.pressure[cell];
            let z = step.z(cell);
            // A step before the wells open has an unset state in the reference; skip anything
            // whose composition does not sum to one.
            if (z.iter().sum::<f64>() - 1.0).abs() > 1e-4 {
                continue;
            }
            // Normalize away the reference's single-precision truncation before flashing: the
            // composition is an *input* here, and the flash validates its sum.
            let total: f64 = z.iter().sum();
            let z: Vec<f64> = z.iter().map(|v| v / total).collect();

            let state = flash(&spec, bar_to_pa(p), t, &z, None)
                .unwrap_or_else(|e| panic!("step {} cell {cell}: {e}", step.sequence));
            compared += 1;

            let reference_two_phase = step.sgas[cell] > 1e-8 && step.soil[cell] > 1e-8;
            if reference_two_phase {
                two_phase += 1;
                assert_eq!(
                    state.phase_state,
                    PhaseState::TwoPhase,
                    "step {} cell {cell}: OPM has S_g = {} and S_o = {}, we say {:?}",
                    step.sequence,
                    step.sgas[cell],
                    step.soil[cell],
                    state.phase_state
                );

                let d = (state.vapour_saturation() - step.sgas[cell]).abs();
                if d > worst_saturation.0 {
                    worst_saturation = (d, format!("step {} cell {cell}", step.sequence));
                }

                let (x, y) = (step.x(cell), step.y(cell));
                for i in 0..3 {
                    for (ours, theirs, tag) in [(state.x[i], x[i], "x"), (state.y[i], y[i], "y")] {
                        let d = (ours - theirs).abs();
                        if d > worst_composition.0 {
                            worst_composition =
                                (d, format!("step {} cell {cell} {tag}{i}", step.sequence));
                        }
                    }
                }
            } else {
                single_phase += 1;
                assert_ne!(
                    state.phase_state,
                    PhaseState::TwoPhase,
                    "step {} cell {cell}: OPM has a single phase (S_g = {}) but we split",
                    step.sequence,
                    step.sgas[cell]
                );
                let d = (state.vapour_saturation() - step.sgas[cell]).abs();
                if d > worst_saturation.0 {
                    worst_saturation = (d, format!("step {} cell {cell}", step.sequence));
                }
            }
        }
    }

    assert!(compared >= 130, "only {compared} states compared");
    assert!(
        two_phase >= 40,
        "only {two_phase} two-phase states in the trajectory"
    );
    assert!(
        single_phase >= 20,
        "only {single_phase} single-phase states"
    );

    // The reference writes single precision, so its own output resolution is ~1e-7. Measured:
    // 1.31e-7 on saturation and 5.55e-8 on composition — i.e. the two implementations agree to the
    // limit of what the fixture can express, and a tighter tolerance would be measuring float32
    // truncation rather than physics.
    assert!(
        worst_saturation.0 < 5e-7,
        "vapour saturation disagrees with OPM: worst absolute error {:e} at {}",
        worst_saturation.0,
        worst_saturation.1
    );
    assert!(
        worst_composition.0 < 5e-7,
        "phase compositions disagree with OPM: worst absolute error {:e} at {}",
        worst_composition.0,
        worst_composition.1
    );
}

/// The initial state, pinned on its own because it is the one every other number descends from and
/// because its agreement is exact to the reference's full printed precision.
#[test]
fn comp_reference_initial_state_matches_opm_to_printed_precision() {
    let reference = reference();
    let spec = deck_fluid();
    let step = &reference.report_steps[0];

    // The deck initializes every cell identically: 75 bar, z = [0.1, 0.3, 0.6].
    for cell in 0..5 {
        assert_eq!(step.pressure[cell], 75.0);
    }

    let state = flash(
        &spec,
        bar_to_pa(75.0),
        spec.reservoir_temperature_k(),
        &[0.1, 0.3, 0.6],
        None,
    )
    .unwrap();
    assert_eq!(state.phase_state, PhaseState::TwoPhase);

    // OPM: 0.29773182, and every digit of x and y it printed.
    assert!(
        (state.vapour_saturation() - step.sgas[0]).abs() < 5e-8,
        "S_g = {} against OPM's {}",
        state.vapour_saturation(),
        step.sgas[0]
    );
    for i in 0..3 {
        assert!(
            (state.x[i] - step.x(0)[i]).abs() < 5e-8,
            "x[{i}] = {} against OPM's {}",
            state.x[i],
            step.x(0)[i]
        );
        assert!(
            (state.y[i] - step.y(0)[i]).abs() < 5e-8,
            "y[{i}] = {} against OPM's {}",
            state.y[i],
            step.y(0)[i]
        );
    }

    // The deck's SGAS = 1 initial value is not the state: the phase split follows from (p, z, T),
    // and both simulators agree it is two-phase at 0.298 gas.
    assert!(state.vapour_saturation() < 0.5);
}

/// Every reference state must also be admissible as a ResSim cell state and must reproduce its own
/// composition. Confirms the state representation (C7) can carry the whole trajectory, not only
/// the states ResSim happens to generate.
#[test]
fn comp_reference_states_round_trip_through_the_cell_representation() {
    let reference = reference();
    let layout = CompositionalLayout::new(3, 5, 2, 2).unwrap();

    for step in &reference.report_steps {
        let z_sums_to_one = (0..5).all(|c| (step.z(c).iter().sum::<f64>() - 1.0).abs() < 1e-4);
        if !z_sums_to_one {
            continue;
        }
        let cells: Vec<CompositionalCellState> = (0..5)
            .map(|c| {
                let z = step.z(c);
                let total: f64 = z.iter().sum();
                CompositionalCellState::new(step.pressure[c], vec![z[0] / total, z[1] / total])
                    .unwrap_or_else(|e| panic!("step {} cell {c}: {e}", step.sequence))
            })
            .collect();
        let state = CompositionalState::new(&layout, cells).unwrap();

        for c in 0..5 {
            let recovered = state.cell(c).overall_composition();
            let z = step.z(c);
            let total: f64 = z.iter().sum();
            for i in 0..3 {
                assert!(
                    (recovered[i] - z[i] / total).abs() < 1e-12,
                    "step {} cell {c} component {i} did not round trip",
                    step.sequence
                );
            }
        }
    }
}

/// Viscosities are computable at every reference state and stay physical. The reference writes
/// them as zeros — `flowexp_comp` does not populate `VOIL`/`VGAS` in its restart — so this is an
/// internal check along an externally supplied trajectory, not a comparison.
#[test]
fn comp_reference_viscosities_are_physical_along_the_trajectory() {
    let reference = reference();
    let spec = deck_fluid();
    let t = spec.reservoir_temperature_k();
    let mut checked = 0;

    for step in &reference.report_steps {
        for cell in 0..reference.cells {
            let z = step.z(cell);
            let total: f64 = z.iter().sum();
            if (total - 1.0).abs() > 1e-4 {
                continue;
            }
            let z: Vec<f64> = z.iter().map(|v| v / total).collect();
            let state = flash(&spec, bar_to_pa(step.pressure[cell]), t, &z, None).unwrap();
            let (mu_l, mu_v) = flash_viscosities(&spec, t, &state).unwrap();
            for mu in [mu_l, mu_v].into_iter().flatten() {
                let cp = crate::fluid::units::pa_s_to_cp(mu);
                assert!(
                    cp > 1e-4 && cp < 10.0,
                    "step {} cell {cell}: viscosity {cp} cP is outside any plausible range",
                    step.sequence
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 150, "only {checked} viscosities evaluated");
}

/// The reference's own well and injection observables, recorded so the transport half of C12 has
/// something to compare against when it lands. Also documents which vectors this simulator does
/// **not** populate, so a future reader does not mistake a column of zeros for agreement.
#[test]
fn comp_reference_summary_carries_usable_well_observables() {
    let reference = reference();

    let bhp_inj = reference.summary.get("WBHP:INJ").expect("WBHP:INJ");
    let bhp_prod = reference.summary.get("WBHP:PROD").expect("WBHP:PROD");
    let gas_injected = reference.summary.get("FGIT").expect("FGIT");
    let time = reference.summary.get("TIME").expect("TIME");

    assert_eq!(bhp_inj.len(), time.len());
    assert!(
        time.last().unwrap() > &20.0,
        "the run should reach about 20 days"
    );

    // The injector starts rate-limited and then sits on its 150 bar BHP limit; the producer holds
    // 50 bar throughout. Both are the deck's controls, read back from the reference.
    assert!(
        bhp_inj.iter().any(|b| (*b - 150.0).abs() < 1e-6),
        "the injector never reached its 150 bar limit"
    );
    assert!(
        bhp_prod
            .iter()
            .filter(|b| **b > 0.0)
            .all(|b| (*b - 50.0).abs() < 1e-6),
        "the producer did not hold 50 bar"
    );

    // Cumulative injection is monotone and ends well above zero.
    let mut previous = 0.0;
    for value in gas_injected {
        assert!(*value >= previous - 1e-9, "cumulative injection decreased");
        previous = *value;
    }
    assert!(previous > 1e5, "only {previous} injected in total");

    // Vectors this simulator does not populate. Asserted so that if a later OPM version starts
    // writing them, the fixture's note stops being true and this test says so.
    for absent in ["FPR"] {
        if let Some(values) = reference.summary.get(absent) {
            assert!(
                values.iter().all(|v| *v == 0.0),
                "{absent} is now populated by flowexp_comp; the fixture's note needs updating"
            );
        }
    }
}
