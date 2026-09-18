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
//! The **transport** comparison is the second half of this file, and it is kept separate on
//! purpose: it needs the deck's transmissibility, well model and timestep control matched as well,
//! and each of those is its own place for two simulators to differ. Conflating them with a
//! thermodynamic comparison would make a disagreement uninterpretable — and in practice it did.
//! Three defects were found only once the two were separated, and one of them (the injector's
//! connection law) was invisible at the settled state and visible only mid-displacement. See
//! `docs/COMPOSITIONAL_VALIDATION.md`'s C12 record.
//!
//! The `comp_refinement_*` tests at the end are the refinement study, `#[ignore]`d and run by
//! `bash scripts/validate-compositional.sh refinement`.
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

// ---------------------------------------------------------------------------------------------
// The transport half: ResSim's own trajectory against the reference's
// ---------------------------------------------------------------------------------------------

use super::accumulation::cell_inventory;
use super::assembly::Face;
use super::flux::Gravity;
use super::relperm::{RelativePermeabilityModel, RelativePermeabilityTable};
use super::state::RockView;
use super::timestep::{CompositionalRun, TimestepOptions};
use super::wells::{Completion, CompositionalWell, SurfacePhase, WellControl};

/// The deck's geometry, as constants so each one can be checked against `1D_COMP.DATA` by eye.
mod deck {
    /// `DARCY_METRIC_FACTOR`, the same constant the black-oil path uses.
    pub const DARCY: f64 = 8.526_988_8e-3;
    pub const CELLS: usize = 5;
    /// `DXV 5*60`, `DYV 6`, `DZV 6`. The refinement study keeps `LENGTH_M` and varies the cell
    /// count, exactly as `tools/opm_compositional/refine_deck.py` does for the reference.
    pub const DX_M: f64 = 60.0;
    pub const LENGTH_M: f64 = CELLS as f64 * DX_M;
    pub const DY_M: f64 = 6.0;
    pub const DZ_M: f64 = 6.0;
    /// `PERMX/Y/Z 5*100`.
    pub const PERM_MD: f64 = 100.0;
    /// `PORO 5*0.1`.
    pub const PORO: f64 = 0.1;
    /// `COMPDAT INJ 1 1 1 1 OPEN 2* 0.0151` — the value after the two defaulted items is
    /// `COMPDAT`'s item 9, which is the wellbore **DIAMETER**, not the radius. (Items 1-8 are
    /// well, I, J, K1, K2, state, saturation table and connection transmissibility factor; the
    /// `2*` defaults the last two of those.) Reading it as a radius doubles `r_w`, which shrinks
    /// `ln(r_eq / r_w)` and so *raises* the well index — by 12% at 10 cells and 15% at 40, since
    /// the error enters through a logarithm and the cell size does not divide out. It read as a
    /// well model that was systematically too injective, and it grew under grid refinement.
    pub const WELL_DIAMETER_M: f64 = 0.0151;
    pub const WELL_RADIUS_M: f64 = WELL_DIAMETER_M / 2.0;
    /// `ROCK 68.9476 0` — reference pressure, and **zero** compressibility.
    pub const ROCK_REFERENCE_BAR: f64 = 68.9476;
    pub const ROCK_COMPRESSIBILITY: f64 = 0.0;
    /// `PRESSURE 5*75.`
    pub const INITIAL_PRESSURE_BAR: f64 = 75.0;
    /// `ZMF` — 0.1 CO2, 0.3 methane, 0.6 decane.
    pub const INITIAL_Z: [f64; 3] = [0.1, 0.3, 0.6];
    /// `WCONINJE INJ GAS OPEN BHP 100000 1* 150` and `WELLSTRE ISTR 1.0 0.0 0.0`.
    ///
    /// The surface gas rate limit is 100 000 sm³/day and the BHP limit is 150 bar. The reference's
    /// `WBHP:INJ` reads 135.29 bar at the first reported time and only later sits at 150, which is
    /// how one can tell the well starts **rate**-limited and switches to BHP when the limit binds.
    /// Modelling it as pure BHP over-injects early and was worth 22 bar of disagreement.
    pub const INJECTOR_SURFACE_RATE_SM3_PER_DAY: f64 = 100_000.0;
    pub const INJECTOR_BHP_LIMIT_BAR: f64 = 150.0;
    pub const INJECTION_STREAM: [f64; 3] = [1.0, 0.0, 0.0];
    /// `WCONPROD PROD OPEN BHP 5* 50`.
    pub const PRODUCER_BHP_BAR: f64 = 50.0;

    /// Cell length on a grid of `cells` cells covering the same `LENGTH_M`.
    pub fn dx_m(cells: usize) -> f64 {
        LENGTH_M / cells as f64
    }

    pub fn pore_volume_m3(cells: usize) -> f64 {
        dx_m(cells) * DY_M * DZ_M * PORO
    }

    /// `DARCY * k * A / L` for a face between two cells of this size.
    pub fn face_transmissibility(cells: usize) -> f64 {
        DARCY * PERM_MD * (DY_M * DZ_M) / dx_m(cells)
    }

    /// Peaceman's equivalent radius for an isotropic rectangular cell, and the geometric well
    /// index that follows. Zero skin, matching `COMPDAT`'s defaults.
    ///
    /// This is where refinement bites hardest on the well: `r_eq` shrinks with the cell, so the
    /// index and the near-well pressure drop both change. Two simulators need not agree on the
    /// coarse grid's `r_eq`, and the C12 startup transient is dominated by that disagreement.
    pub fn well_index(cells: usize) -> f64 {
        let dx = dx_m(cells);
        let r_eq = 0.28 * (dx * dx + DY_M * DY_M).sqrt() / 2.0;
        DARCY * 2.0 * std::f64::consts::PI * PERM_MD * DZ_M / (r_eq / WELL_RADIUS_M).ln()
    }
}

/// The deck's `SGOF` table, as a ResSim relative permeability model.
///
/// `SGOF` is keyed on **gas** saturation and this model is keyed on **liquid** saturation, so the
/// rows are reversed and `S_L = 1 - S_g`. The table is Corey-squared — `krg = Sg^2`,
/// `kro = (1-Sg)^2` — but it is transcribed as a table rather than recognised as a Corey curve,
/// because what the deck supplies is a table and the reference interpolated it.
fn deck_relperm() -> RelativePermeabilityModel {
    let mut liquid_saturation = Vec::new();
    let mut kr_liquid = Vec::new();
    let mut kr_vapour = Vec::new();
    // 0.00 to 1.00 in steps of 0.05, as the deck lists it, reversed into liquid saturation.
    for step in (0..=20).rev() {
        let s_g = step as f64 * 0.05;
        liquid_saturation.push(1.0 - s_g);
        kr_liquid.push((1.0 - s_g) * (1.0 - s_g));
        kr_vapour.push(s_g * s_g);
    }
    RelativePermeabilityModel::Tabulated(
        RelativePermeabilityTable::new(liquid_saturation, kr_liquid, kr_vapour)
            .expect("the deck's SGOF table must validate"),
    )
}

/// The transcribed `SGOF` table must reproduce the deck's rows, including the reversal.
#[test]
fn comp_reference_deck_relperm_table_matches_sgof() {
    let m = deck_relperm();
    // Deck rows: Sg, Krg, Kro.
    for (s_g, krg, kro) in [
        (0.0, 0.0, 1.0),
        (0.25, 0.0625, 0.5625),
        (0.5, 0.25, 0.25),
        (0.75, 0.5625, 0.0625),
        (1.0, 1.0, 0.0),
    ] {
        let s_l = 1.0 - s_g;
        assert!(
            (m.kr_vapour::<f64>(s_l) - krg).abs() < 1e-12,
            "Sg = {s_g}: krg = {} but the deck says {krg}",
            m.kr_vapour::<f64>(s_l)
        );
        assert!(
            (m.kr_liquid::<f64>(s_l) - kro).abs() < 1e-12,
            "Sg = {s_g}: kro = {} but the deck says {kro}",
            m.kr_liquid::<f64>(s_l)
        );
    }
    assert!(
        !m.is_verification_only(),
        "a deck-supplied table is not a verification model"
    );
}

/// Largest sub-step the driver will take between report times.
///
/// Not the deck's `TSTEP` ladder: matching report times is what a trajectory comparison needs, and
/// matching a timestep ladder would be comparing two timestep controllers rather than two models.
const MAX_SUB_STEP_DAYS: f64 = 0.05;

/// When the deck's wells start flowing.
///
/// `1D_COMP.DATA` puts four `TSTEP`s — 0.01, 0.02, 0.04, 0.04 days — *before* `WELSPECS`, so
/// nothing happens for the first 0.11 days. The reference's first four report steps are therefore
/// the initial state unchanged, and a driver that opened the wells at t = 0 would be a whole
/// displacement ahead before the comparison started.
const WELLS_OPEN_DAYS: f64 = 0.11;

/// How close to [`WELLS_OPEN_DAYS`] counts as having reached it.
///
/// **The reference's `TIME` vector is single precision** — its last entry reads `20.1100006` — so
/// the report time that should be 0.11 arrives as `0.10999999940395355`, and the driver steps to
/// exactly that. A tolerance of `1e-12` therefore said the wells were still shut at the moment
/// they should have opened, and they stayed shut for one whole sub-step. That cost 0.05 of the
/// first 0.08 days of injection, which is 60% of the first report interval, and it showed up as a
/// cumulative-injection error that *grew* as the sub-step was refined — the signature of a fixed
/// amount of missing time being resolved away rather than of a converging discretisation.
///
/// 1e-6 days is 0.09 s: far above float32's resolution at 0.11 and far below any timestep here.
const WELLS_OPEN_TOLERANCE_DAYS: f64 = 1e-6;

/// Build the deck's case in ResSim and advance it to each of the reference's report times.
///
/// Returns the state at each reported time, in the reference's own order.
fn run_deck_case(report_times_days: &[f64]) -> Vec<Vec<CompositionalCellState>> {
    run_deck_case_with_totals(report_times_days).1
}

/// As [`run_deck_case`], on the deck's own five-cell grid at the default sub-step.
fn run_deck_case_with_totals(
    report_times_days: &[f64],
) -> (Vec<Vec<f64>>, Vec<Vec<CompositionalCellState>>) {
    run_deck_case_on_grid(deck::CELLS, MAX_SUB_STEP_DAYS, report_times_days)
}

/// The same case on `cells` cells with a largest sub-step of `max_sub_step_days`.
///
/// Both knobs are what the refinement study varies. The grid matches
/// `tools/opm_compositional/refine_deck.py`: same length, same rock, same fluid, same wells in the
/// first and last cell, only the discretisation changes.
fn run_deck_case_on_grid(
    cells: usize,
    max_sub_step_days: f64,
    report_times_days: &[f64],
) -> (Vec<Vec<f64>>, Vec<Vec<CompositionalCellState>>) {
    let spec = deck_fluid();
    let relperm = deck_relperm();
    let layout = CompositionalLayout::new(3, cells, 2, 2).unwrap();

    let pore_volumes = vec![deck::pore_volume_m3(cells); cells];
    let rock = RockView {
        pore_volume_ref_m3: &pore_volumes,
        reference_pressure_bar: deck::ROCK_REFERENCE_BAR,
        compressibility_per_bar: deck::ROCK_COMPRESSIBILITY,
    };

    let faces: Vec<Face> = (0..cells - 1)
        .map(|i| Face {
            cell_i: i,
            cell_j: i + 1,
            geom_t: deck::face_transmissibility(cells),
            // The deck is a single horizontal layer, so gravity does nothing along the column.
            gravity: Gravity::OFF,
        })
        .collect();

    let initial = CompositionalState::new(
        &layout,
        (0..cells)
            .map(|_| {
                CompositionalCellState::new(
                    deck::INITIAL_PRESSURE_BAR,
                    vec![deck::INITIAL_Z[0], deck::INITIAL_Z[1]],
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();

    let injector = CompositionalWell {
        id: "INJ".to_string(),
        completions: vec![Completion {
            cell: 0,
            well_index: deck::well_index(cells),
            head_offset_bar: 0.0,
        }],
        control: WellControl::SurfaceRate {
            target_m3_per_day: deck::INJECTOR_SURFACE_RATE_SM3_PER_DAY,
            // `SurfacePhase::Total`, not `Vapour`, and the reason is a real caveat rather than a
            // convenience. The injected stream is pure CO2, which at the deck's surface conditions
            // (1 bar, 15 °C) is **below its critical temperature** of 304 K — so Li's phase
            // labelling, which the flash uses for single-phase states and which ignores pressure
            // entirely, calls it a liquid. A vapour-rate target therefore reads zero and the
            // control concludes its BHP limit binds when it does not.
            //
            // The stream is single phase at surface, so `Total` is the unambiguous expression of
            // "100 000 sm³/day of what this well injects". See
            // `comp_reference_surface_phase_label_is_pressure_blind`.
            phase: SurfacePhase::Total,
            bhp_limit_bar: deck::INJECTOR_BHP_LIMIT_BAR,
        },
        injection_composition: Some(deck::INJECTION_STREAM.to_vec()),
    };
    let producer = CompositionalWell {
        id: "PROD".to_string(),
        completions: vec![Completion {
            cell: cells - 1,
            well_index: deck::well_index(cells),
            head_offset_bar: 0.0,
        }],
        control: WellControl::Bhp {
            target_bar: deck::PRODUCER_BHP_BAR,
        },
        injection_composition: None,
    };

    let open_wells = vec![injector, producer];
    let mut run = CompositionalRun::new(initial);
    let mut snapshots = Vec::with_capacity(report_times_days.len());
    let mut cumulative = Vec::with_capacity(report_times_days.len());
    // A looser Newton tolerance than the default 1e-8, and the reason is specific rather than
    // convenient. Late in this displacement the cells are almost pure CO2, so methane and decane
    // sit at ~1e-5 of the cell's inventory; C8's per-component row scaling then divides their
    // balances by their own tiny inventories, and the achievable residual is bounded by the
    // flash's own 1e-11 equilibrium tolerance rather than by the Newton step. Measured: the solve
    // reaches 1.8e-8 and stalls there. Demanding 1e-8 of a component holding 1e-5 of the material
    // is demanding more than the thermodynamics under it can deliver.
    //
    // This is a property of the case, not a default worth changing: a trace component that cannot
    // converge tightly is exactly what the per-component scaling is supposed to make visible.
    let options = TimestepOptions {
        newton: super::newton::NewtonOptions {
            tolerance: 1e-7,
            max_iterations: 30,
        },
        ..TimestepOptions::default()
    };

    for &target in report_times_days {
        // Advance to the report time, sub-stepping as needed. The deck's own TSTEP list is not
        // reproduced: matching report times is what a trajectory comparison needs, and matching
        // a timestep ladder would be comparing two timestep controllers rather than two models.
        let mut guard = 0;
        // Stop once the remaining time is below the smallest step worth taking. Summing many
        // sub-steps accumulates enough floating-point drift that the last remainder can be a few
        // times 1e-7, and asking the lifecycle for a step below its own minimum is a budget
        // failure rather than a tiny step.
        while target - run.time_days() > options.min_dt_days {
            guard += 1;
            assert!(guard < 5000, "sub-stepping did not reach {target} days");
            let remaining = target - run.time_days();
            let dt = remaining.min(max_sub_step_days);

            // The deck declares its wells *after* its first four TSTEPs, so nothing flows for the
            // first 0.11 days. Reproducing that matters: applying the wells from t = 0 would put
            // ResSim a whole displacement ahead of the reference before the comparison began.
            let wells: &[CompositionalWell] =
                if run.time_days() >= WELLS_OPEN_DAYS - WELLS_OPEN_TOLERANCE_DAYS {
                    &open_wells
                } else {
                    &[]
                };
            // The wells go in as wells, not as a precomputed source. See `assembly`'s module docs:
            // an explicitly evaluated BHP well has no pressure feedback and overshoots its own BHP.
            let sources = vec![vec![0.0; 3]; cells];

            let report = run.step(
                &spec, &layout, &rock, &relperm, &faces, wells, &sources, dt, options,
            );
            assert!(
                report.succeeded(),
                "step at {} days failed: {:?}",
                run.time_days(),
                report.failure
            );
        }
        snapshots.push(run.state().cells().to_vec());
        cumulative.push(
            run.cumulative_well_moles()
                .first()
                .cloned()
                .unwrap_or_else(|| vec![0.0; 3]),
        );
    }
    (cumulative, snapshots)
}

/// **The transport half of C12.** Run the deck's case in ResSim and compare the trajectory
/// against `flowexp_comp`'s.
///
/// This is a different kind of comparison from the flash one above. It exercises the whole model —
/// transmissibility, upwinding, the well connection law and its controls, the Newton lifecycle and
/// the timestep controller — against a simulator that shares none of that code.
///
/// The agreement is reported in three places rather than one, because a single worst-case number
/// over a five-cell displacement says almost nothing:
///
/// * **the startup transient**, where the two differ most, and where the difference is dominated by
///   a different Peaceman equivalent radius and a different timestep ladder rather than by the
///   model;
/// * **the developed displacement**, after the front has left the injection cell;
/// * **the final state**, where both simulators have settled and the comparison is of two steady
///   solutions rather than of two transients.
#[test]
fn comp_reference_transport_trajectory_tracks_opm() {
    let reference = reference();
    let times = reference.summary.get("TIME").expect("TIME").clone();
    assert_eq!(times.len(), reference.report_steps.len());

    let ours = run_deck_case(&times);

    let mut worst_pressure = (0.0f64, String::new());
    let mut worst_pressure_developed = (0.0f64, String::new());
    let mut worst_co2 = (0.0f64, String::new());
    let mut compared = 0usize;

    for (index, step) in reference.report_steps.iter().enumerate() {
        let developed = times[index] >= 2.0;
        for cell in 0..deck::CELLS {
            let theirs_p = step.pressure[cell];
            let ours_p = ours[index][cell].pressure_bar;
            let d = (ours_p - theirs_p).abs();
            let where_ = format!(
                "t = {:.2} d, cell {cell}: {ours_p:.3} vs {theirs_p:.3} bar",
                times[index]
            );
            if d > worst_pressure.0 {
                worst_pressure = (d, where_.clone());
            }
            if developed && d > worst_pressure_developed.0 {
                worst_pressure_developed = (d, where_);
            }

            let theirs_z = step.z(cell);
            let total: f64 = theirs_z.iter().sum();
            if (total - 1.0).abs() > 1e-4 {
                continue;
            }
            let ours_z = ours[index][cell].overall_composition();
            let d = (ours_z[0] - theirs_z[0] / total).abs();
            if d > worst_co2.0 {
                worst_co2 = (
                    d,
                    format!(
                        "t = {:.2} d, cell {cell}: z_CO2 {:.5} vs {:.5}",
                        times[index],
                        ours_z[0],
                        theirs_z[0] / total
                    ),
                );
            }
            compared += 1;
        }
    }

    // The final state: both simulators have settled, so this compares two steady solutions rather
    // than two transients, and it is the most meaningful single number here.
    let last = reference.report_steps.last().unwrap();
    let mut worst_final = 0.0f64;
    for cell in 0..deck::CELLS {
        worst_final = worst_final.max((ours[27][cell].pressure_bar - last.pressure[cell]).abs());
    }

    assert!(compared >= 130, "only {compared} states compared");

    // Printed so the recorded baseline can be reproduced rather than trusted. Run with
    // `cargo test comp_reference_transport_trajectory_tracks_opm -- --nocapture`.
    eprintln!(
        "C12 transport over {compared} states:\n           final state:            {worst_final:.3} bar\n           developed displacement: {:.3} bar at {}\n           startup transient:      {:.3} bar at {}\n           CO2 front:              {:.4} at {}",
        worst_pressure_developed.0,
        worst_pressure_developed.1,
        worst_pressure.0,
        worst_pressure.1,
        worst_co2.0,
        worst_co2.1
    );

    // Measured, not aspirational, and each bound sits just above what this case produces.
    assert!(
        worst_final < 0.05,
        "the final states disagree by {worst_final:.3} bar; both simulators have settled by then"
    );
    assert!(
        worst_pressure_developed.0 < 4.5,
        "the developed displacement diverges: worst {:.3} bar at {}",
        worst_pressure_developed.0,
        worst_pressure_developed.1
    );
    // The startup transient, which is no longer the worst part of the trajectory. It was 20 bar
    // until two driver defects were found by the refinement study: the wells opened one sub-step
    // late because the reference's report times are single precision, and the wellbore radius was
    // read from `COMPDAT` item 9, which is a diameter. See `deck::WELL_DIAMETER_M` and
    // `WELLS_OPEN_TOLERANCE_DAYS`.
    assert!(
        worst_pressure.0 < 4.5,
        "the startup transient diverges: worst {:.3} bar at {}",
        worst_pressure.0,
        worst_pressure.1
    );
    // The CO2 front. On five cells, numerical diffusion is large and a small difference in front
    // arrival time reads as a large composition difference in whichever cell the front is crossing.
    assert!(
        worst_co2.0 < 0.14,
        "the CO2 front diverges: worst {:.4} at {}",
        worst_co2.0,
        worst_co2.1
    );
}

/// Cumulative injection against the reference's `FGIT` — the plan's actual acceptance target for
/// C12 is "≤ 1% for cumulative quantities on smooth simple fixtures", and a cumulative is the right
/// observable because it integrates over the transients that dominate the pointwise comparison.
///
/// The comparison is of **moles**, converted from the reference's surface volumes at the deck's own
/// `STCOND`. Comparing surface volumes directly would fold in the surface-flash labelling caveat
/// documented in `comp_reference_surface_phase_label_is_pressure_blind`.
#[test]
fn comp_reference_cumulative_injection_tracks_opm() {
    let reference = reference();
    let times = reference.summary.get("TIME").expect("TIME").clone();
    let fgit = reference.summary.get("FGIT").expect("FGIT").clone();

    let spec = deck_fluid();
    let surface = spec.surface().unwrap();

    // The reference reports cumulative injection as a surface volume. One mole of the injected
    // pure CO2 occupies this much at the deck's surface conditions.
    let injected_state = flash(
        &spec,
        surface.pressure_pa,
        surface.temperature_k,
        &deck::INJECTION_STREAM,
        None,
    )
    .unwrap();
    let molar_volume = injected_state.mixture_molar_volume();

    let (ours_moles, _) = run_deck_case_with_totals(&times);

    // Compare at the end, where the cumulative is largest and the relative error most meaningful.
    let theirs_moles = fgit.last().unwrap() / molar_volume;
    let ours_total = ours_moles.last().unwrap()[0];
    let relative = (ours_total - theirs_moles).abs() / theirs_moles;

    eprintln!(
        "C12 cumulative injection: {ours_total:.6e} vs {theirs_moles:.6e} moles, {:.2}%",
        relative * 100.0
    );

    assert!(
        theirs_moles > 1e6,
        "the reference injected only {theirs_moles} moles; the fixture looks wrong"
    );
    // 1.10%, measured, and **not** a clean acceptance result even though it is under the plan's
    // 1%... on this grid alone. Under refinement it grows to 5.1%, because the injector ends up
    // running at about one bar of drawdown against its 150 bar limit and the rate observable
    // therefore amplifies the pressure observable by roughly a hundred. See
    // `comp_refinement_cumulative_injection_is_bounded_by_its_own_conditioning`, which measures
    // that. The bound here is a regression guard on the deck's own grid, not a claim that the
    // target is met.
    assert!(
        relative < 0.015,
        "cumulative injection differs by {:.2}%: {ours_total:.4e} vs {theirs_moles:.4e} moles",
        relative * 100.0
    );
}

/// Li's single-phase labelling ignores pressure, and at surface conditions that misclassifies a
/// component below its critical temperature.
///
/// Pure CO2 at 1 bar and 15 °C is unambiguously a gas — its vapour pressure there is about 50 bar —
/// but `T = 288.15 K` is below `Tc = 304.128 K`, and Li's criterion is exactly `T < Tc_est`. The
/// flash therefore labels it a liquid.
///
/// **This is a real limitation, not a bug in the flash.** Li's method is what OPM uses and what C3
/// reproduces on all 47 flashed states; it is a cheap label that is sound at reservoir pressure and
/// unsound at atmospheric. The consequence is concrete: a surface **gas**-rate control on a
/// CO2-rich stream reads zero, and a well concludes its BHP limit binds when it does not. C13's
/// reporting has to name what it means by surface gas rather than inherit this label.
#[test]
fn comp_reference_surface_phase_label_is_pressure_blind() {
    let spec = deck_fluid();
    let surface = spec.surface().unwrap();

    // Pure CO2 at the deck's surface conditions.
    let state = flash(
        &spec,
        surface.pressure_pa,
        surface.temperature_k,
        &[1.0, 0.0, 0.0],
        None,
    )
    .unwrap();
    assert_eq!(
        state.phase_state,
        PhaseState::SingleLiquid,
        "the premise of this test has changed: Li no longer labels surface CO2 a liquid"
    );

    // It is a gas by every physical measure: its molar volume is within a few per cent of the
    // ideal-gas value at 1 bar.
    let ideal =
        crate::fluid::units::GAS_CONSTANT_J_PER_MOL_K * surface.temperature_k / surface.pressure_pa;
    let actual = 1.0 / state.liquid.as_ref().unwrap().molar_density;
    assert!(
        (actual - ideal).abs() / ideal < 0.02,
        "surface CO2 has molar volume {actual} against an ideal {ideal}; it is a gas"
    );

    // And `T < Tc` is exactly why the label says otherwise.
    assert!(surface.temperature_k < spec.component(0).critical_temperature_k);

    // The operational consequence: a vapour-rate measure of this stream is zero.
    let separated =
        crate::fluid::transport::surface_separation(&spec, &[1000.0, 0.0, 0.0]).unwrap();
    assert_eq!(separated.vapour_volume, 0.0);
    assert!(separated.liquid_volume > 0.0);
    // `Total` is unambiguous whatever the label says, which is why the C12 driver uses it.
    assert!(separated.liquid_volume + separated.vapour_volume > 0.0);
}

// ---------------------------------------------------------------------------------------------
// C12's refinement study: does the disagreement shrink, and is it spatial or temporal?
// ---------------------------------------------------------------------------------------------
//
// The plan asks for "at least three timestep resolutions, then spatial refinement separately"
// and for the comparison to be against "a refined external solution, not merely a coarse Flow
// output". Both halves matter for a different reason:
//
// * **Timestep refinement** establishes that the comparison sub-step is already small enough that
//   the remaining disagreement is not the timestep controller. Without it, a spatial convergence
//   claim is confounded.
// * **Grid refinement** is the actual test of the discretisation. `flowexp_comp` is re-run at 10,
//   20 and 40 cells over the same 300 m by `tools/opm_compositional/run-refinement.sh`, so at each
//   resolution ResSim is compared against a reference solved on *that* grid.
//
// These are `#[ignore]`d and run by `bash scripts/validate-compositional.sh refinement`, which
// builds in release. The plan allows exactly that: "bounded `comp_reference_*`; longer cases
// explicitly ignored with a dedicated release runner."

/// The reference solved on `cells` cells. 5 is the deck's own grid; the rest are refined.
fn reference_on(cells: usize) -> Reference {
    let raw = match cells {
        5 => include_str!("../../../../../opm/compositional/1d_comp/reference.json"),
        10 => include_str!("../../../../../opm/compositional/1d_comp/refined/n010/reference.json"),
        20 => include_str!("../../../../../opm/compositional/1d_comp/refined/n020/reference.json"),
        40 => include_str!("../../../../../opm/compositional/1d_comp/refined/n040/reference.json"),
        other => panic!("no reference fixture for {other} cells; see run-refinement.sh"),
    };
    let r: Reference = serde_json::from_str(raw).expect("the reference fixture must parse");
    assert_eq!(r.schema, "ressim-compositional-reference/1");
    assert_eq!(r.cells, cells, "fixture is for the wrong grid");
    assert_eq!(r.report_steps.len(), 28);
    r
}

/// Total component moles held by a grid, from each cell's `(p, z)`.
///
/// **This is the conversion-free observable.** Comparing cumulative injection needs each
/// simulator's surface volume, and the two need not mean the same thing by a cubic metre of
/// surface CO2 — the reference reports `FGIT` through its own surface flash, and ResSim's surface
/// flash labels the same stream a liquid (see
/// `comp_reference_surface_phase_label_is_pressure_blind`). This observable sidesteps that
/// entirely: it compares what both simulators actually conserve, moles in the reservoir.
///
/// Both inventories are evaluated with **ResSim's** EOS, from each simulator's own `(p, z)`. That
/// is legitimate precisely because C12's thermodynamic half already showed the two EOS
/// implementations agree on this fixture to 1e-7 — the molar density is not the thing under test
/// here, the transport is.
fn grid_inventory(
    spec: &FluidSpecification,
    rock: &RockView<'_>,
    cells: &[CompositionalCellState],
) -> Vec<f64> {
    let n = cells[0].overall_composition().len();
    let mut total = vec![0.0; n];
    for (index, cell) in cells.iter().enumerate() {
        let (inventory, _) =
            cell_inventory(spec, rock, index, cell).expect("the inventory must flash");
        for (i, moles) in inventory.component_moles.iter().enumerate() {
            total[i] += moles;
        }
    }
    total
}

/// The reference's own state at `step`, as ResSim cell states, so the same inventory code sees it.
fn reference_cells(step: &ReferenceStep, cells: usize) -> Vec<CompositionalCellState> {
    (0..cells)
        .map(|cell| {
            let z = step.z(cell);
            // The fixture is single precision, so its `z` sums to 1 only to about 1e-7.
            let total: f64 = z.iter().sum();
            CompositionalCellState::new(step.pressure[cell], vec![z[0] / total, z[1] / total])
                .expect("the reference's own state must be admissible")
        })
        .collect()
}

/// What one point of the refinement ladder measures.
///
/// Pressures are compared at the **final** report time, where both simulators have settled, so the
/// number is a difference between two steady solutions rather than between two transients. The
/// cumulative is the plan's own acceptance observable.
#[derive(Debug, Clone, Copy)]
struct RefinementPoint {
    cells: usize,
    sub_step_days: f64,
    /// Worst absolute pressure difference over the grid at the last report time, bar.
    final_pressure_bar: f64,
    /// Worst absolute pressure difference over the whole trajectory, bar.
    trajectory_pressure_bar: f64,
    /// Relative difference in cumulative injected moles at the last report time, through the
    /// reference's surface volumes. Carries the surface-conversion caveat.
    cumulative_relative: f64,
    /// Relative difference in **CO2 moles held by the grid** at the last report time.
    /// Conversion-free: see [`grid_inventory`].
    inventory_relative: f64,
    /// The reference's own injector drawdown at the last report time, `BHP - p_cell` [bar].
    ///
    /// This is what conditions the rate comparison. An injector on its BHP limit has
    /// `q = WI * lambda * (BHP - p)`, so a pressure disagreement of `dp` is a *relative* rate
    /// disagreement of `dp / (BHP - p)`. Late in this case that denominator is about one bar, so
    /// the rate observable amplifies the pressure observable by roughly a hundred.
    injector_drawdown_bar: f64,
    /// Worst absolute pressure difference in the injection cell over the second half of the run,
    /// where most of the injection happens [bar].
    late_injection_cell_bar: f64,
}

/// Run ResSim on `cells` cells at `sub_step_days` and compare against the reference solved on the
/// same grid.
fn measure_refinement(cells: usize, sub_step_days: f64) -> RefinementPoint {
    let reference = reference_on(cells);
    let times = reference.summary.get("TIME").expect("TIME").clone();
    let fgit = reference.summary.get("FGIT").expect("FGIT").clone();

    let (ours_moles, ours) = run_deck_case_on_grid(cells, sub_step_days, &times);

    let mut trajectory_pressure_bar = 0.0f64;
    for (index, step) in reference.report_steps.iter().enumerate() {
        for cell in 0..cells {
            trajectory_pressure_bar = trajectory_pressure_bar
                .max((ours[index][cell].pressure_bar - step.pressure[cell]).abs());
        }
    }

    // The injection cell over the second half, which is where most of the cumulative accrues.
    let mut late_injection_cell_bar = 0.0f64;
    for (index, step) in reference.report_steps.iter().enumerate() {
        if times[index] >= 10.0 {
            late_injection_cell_bar =
                late_injection_cell_bar.max((ours[index][0].pressure_bar - step.pressure[0]).abs());
        }
    }

    let last = reference.report_steps.last().unwrap();
    let mut final_pressure_bar = 0.0f64;
    for cell in 0..cells {
        final_pressure_bar = final_pressure_bar
            .max((ours.last().unwrap()[cell].pressure_bar - last.pressure[cell]).abs());
    }

    // The reference reports cumulative injection as a surface volume; compare in moles, which is
    // what both simulators actually conserve. See `comp_reference_cumulative_injection_tracks_opm`.
    let spec = deck_fluid();
    let surface = spec.surface().unwrap();
    let molar_volume = flash(
        &spec,
        surface.pressure_pa,
        surface.temperature_k,
        &deck::INJECTION_STREAM,
        None,
    )
    .unwrap()
    .mixture_molar_volume();
    let theirs_moles = fgit.last().unwrap() / molar_volume;
    let ours_total = ours_moles.last().unwrap()[0];

    // The conversion-free comparison: CO2 held by the grid at the last report time.
    let pore_volumes = vec![deck::pore_volume_m3(cells); cells];
    let rock = RockView {
        pore_volume_ref_m3: &pore_volumes,
        reference_pressure_bar: deck::ROCK_REFERENCE_BAR,
        compressibility_per_bar: deck::ROCK_COMPRESSIBILITY,
    };
    let theirs_inventory = grid_inventory(
        &spec,
        &rock,
        &reference_cells(reference.report_steps.last().unwrap(), cells),
    );
    let ours_inventory = grid_inventory(&spec, &rock, ours.last().unwrap());

    RefinementPoint {
        cells,
        sub_step_days,
        final_pressure_bar,
        trajectory_pressure_bar,
        cumulative_relative: (ours_total - theirs_moles).abs() / theirs_moles,
        inventory_relative: (ours_inventory[0] - theirs_inventory[0]).abs() / theirs_inventory[0],
        injector_drawdown_bar: deck::INJECTOR_BHP_LIMIT_BAR - last.pressure[0],
        late_injection_cell_bar,
    }
}

fn print_ladder(label: &str, points: &[RefinementPoint]) {
    eprintln!("{label}");
    eprintln!(
        "  cells  sub-step/d   final p/bar   worst p/bar   CO2 in place   cumulative   \
         drawdown/bar   late p0/bar   amplified"
    );
    for p in points {
        eprintln!(
            "  {:5}  {:10.4}   {:11.3}   {:11.3}   {:11.3}%   {:8.3}%   {:12.3}   {:11.3}   \
             {:8.3}%",
            p.cells,
            p.sub_step_days,
            p.final_pressure_bar,
            p.trajectory_pressure_bar,
            p.inventory_relative * 100.0,
            p.cumulative_relative * 100.0,
            p.injector_drawdown_bar,
            p.late_injection_cell_bar,
            100.0 * p.late_injection_cell_bar / p.injector_drawdown_bar
        );
    }
}

/// Successive absolute differences down a ladder, for reading a convergence rate by eye.
fn differences(values: &[f64]) -> Vec<f64> {
    values.windows(2).map(|w| (w[1] - w[0]).abs()).collect()
}

/// **Timestep refinement**, run first because a spatial claim is confounded without it.
///
/// Halving the sub-step four times over. What this establishes is that the comparison sub-step is
/// on the converged part of the curve, so the disagreement reported by
/// `comp_reference_transport_trajectory_tracks_opm` is not the timestep controller.
#[test]
#[ignore = "release runner: bash scripts/validate-compositional.sh refinement"]
fn comp_refinement_timestep_is_converged_at_the_comparison_step() {
    let points: Vec<_> = [0.1, 0.05, 0.025, 0.0125]
        .into_iter()
        .map(|dt| measure_refinement(10, dt))
        .collect();
    print_ladder("C12 timestep refinement, 10 cells:", &points);

    // The settled state does not move at all with the sub-step: the same steady solution is
    // reached however it is approached.
    for p in &points {
        assert!(
            (p.final_pressure_bar - points[0].final_pressure_bar).abs() < 1e-3,
            "the final state moved with the sub-step: {:?}",
            points
        );
        assert!(
            (p.inventory_relative - points[0].inventory_relative).abs() < 1e-4,
            "CO2 in place moved with the sub-step: {:?}",
            points
        );
    }

    // The transient does move with the sub-step, and it converges. Cumulative injection is an
    // integral over the whole run, so its ladder is smooth enough to read a rate from: each
    // halving changes it by about half as much as the previous one, which is the first order a
    // backward-Euler step gives.
    let cumulative = differences(
        &points
            .iter()
            .map(|p| p.cumulative_relative)
            .collect::<Vec<_>>(),
    );
    assert!(
        cumulative[1] < 0.6 * cumulative[0] && cumulative[2] < 0.6 * cumulative[1],
        "cumulative injection is not converging at first order in the sub-step: {cumulative:?}"
    );

    // The worst pointwise pressure difference is a maximum over a discrete set of cells and report
    // times, so which state attains it can change from one resolution to the next. It is required
    // to settle, not to halve — reading a convergence rate off a max would be reading one off the
    // grid's own sampling.
    let transient = differences(
        &points
            .iter()
            .map(|p| p.trajectory_pressure_bar)
            .collect::<Vec<_>>(),
    );
    assert!(
        transient[1] < transient[0] && transient[2] < transient[1],
        "the transient is not settling with the sub-step: successive changes {transient:?}"
    );
    assert!(
        transient[2] < 0.5,
        "the transient is still moving by {:.3} bar per halving; the comparison sub-step is not \
         on the converged part of the curve",
        transient[2]
    );
}

/// **Grid refinement** against a reference re-solved on each grid.
///
/// The state observables — the settled pressure field and the CO2 the grid holds — agree at every
/// resolution and do not drift with it. That is the load-bearing result: two independent
/// implementations of compositional transport put the same material in the same places.
#[test]
#[ignore = "release runner: bash scripts/validate-compositional.sh refinement"]
fn comp_refinement_grid_agreement_holds_on_state_observables() {
    let points: Vec<_> = [5, 10, 20, 40]
        .into_iter()
        .map(|cells| measure_refinement(cells, MAX_SUB_STEP_DAYS))
        .collect();
    print_ladder("C12 grid refinement, sub-step 0.05 d:", &points);

    for p in &points {
        assert!(
            p.final_pressure_bar < 0.05,
            "{} cells: the settled pressures disagree by {:.4} bar",
            p.cells,
            p.final_pressure_bar
        );
        assert!(
            p.inventory_relative < 5e-4,
            "{} cells: CO2 in place disagrees by {:.4}%",
            p.cells,
            p.inventory_relative * 100.0
        );
    }

    // The transient is where a five-cell grid and a forty-cell grid genuinely differ, and it does
    // not shrink: refining resolves a sharper front, and a sharper front makes a small difference
    // in arrival time read as a larger pointwise difference. Bounded, not converging.
    for p in &points {
        assert!(
            p.trajectory_pressure_bar < 9.0,
            "{} cells: the trajectory diverges by {:.3} bar",
            p.cells,
            p.trajectory_pressure_bar
        );
    }
}

/// **Cumulative injection does not meet the plan's 1% target, and this measures why.**
///
/// The plan asks for <= 1% on cumulative quantities. Measured here: 1.10% on the deck's own grid,
/// rising to 5.12% at forty cells. It is not met, and the reason is not a mystery — it is the
/// conditioning of the observable.
///
/// Late in this case the injector sits on its 150 bar limit against a cell at about 149 bar, so
/// the drawdown driving it is around one bar and falls as the grid is refined (2.03 bar at five
/// cells, 1.15 at forty — a smaller injection cell fills closer to the BHP). An injector on its
/// limit has `q = WI * lambda * (BHP - p)`, so a pressure disagreement of `dp` is a *relative*
/// rate disagreement of `dp / (BHP - p)`. **The rate observable amplifies the pressure observable
/// by about a hundred, and the amplification grows under refinement.**
///
/// That is exactly what the ladder shows. The injection cell's pressure agrees to about 0.1 bar
/// over the second half of the run, where most of the cumulative accrues; divided by the drawdown
/// that is 6.1% at five cells and 8.6% at forty, and the measured cumulative disagreement sits
/// below that bound at every resolution while growing in step with it.
///
/// So the two observables are consistent: a state agreement any tighter than 0.01 bar would be
/// needed to bring the cumulative under 1% here, and the state agreement is already at 0.03 bar on
/// the settled field and 0.02% on the CO2 the grid holds. **The target is not met, the reason is
/// quantified, and the milestone stays undeclared.**
///
/// Two things this is *not*:
///
/// * It is not the surface-volume conversion. `FGIT` is a surface volume and ResSim's surface
///   flash has a known limitation at 1 bar (`comp_reference_surface_phase_label_is_pressure_blind`),
///   so that was the first suspect. It was checked directly: ResSim's connection law applied to the
///   reference's *own* final cell pressure reproduces the reference's `FGIR` to 0.4%, through that
///   same conversion. A fixed conversion error also cannot produce a grid-dependent relative error.
/// * It is not the transport. CO2 held by the grid agrees to better than 0.02% at every resolution,
///   conversion-free. After breakthrough the injected CO2 passes straight through, so throughput is
///   invisible in the in-place amount — which is why both observables are needed and neither
///   substitutes for the other.
///
/// **What would make this a meaningful discriminator:** a case whose injector is not near shut-in,
/// so the cumulative is not a small difference of large numbers. `1D_COMP` is not that case.
#[test]
#[ignore = "release runner: bash scripts/validate-compositional.sh refinement"]
fn comp_refinement_cumulative_injection_is_bounded_by_its_own_conditioning() {
    let points: Vec<_> = [5, 10, 20, 40]
        .into_iter()
        .map(|cells| measure_refinement(cells, MAX_SUB_STEP_DAYS))
        .collect();
    print_ladder(
        "C12 cumulative injection against its conditioning:",
        &points,
    );

    for p in &points {
        // The amplification is measured, not assumed, so the argument above is evidence.
        let amplification = 1.0 / p.injector_drawdown_bar;
        assert!(
            amplification > 0.4,
            "{} cells: the injector drawdown is {:.3} bar, so the rate observable is no longer \
             ill-conditioned and this test's reasoning no longer applies",
            p.cells,
            p.injector_drawdown_bar
        );

        let bound = p.late_injection_cell_bar * amplification;
        assert!(
            p.cumulative_relative < bound,
            "{} cells: cumulative injection differs by {:.3}%, more than the {:.3}% that the \
             injection cell's own pressure agreement ({:.3} bar over a {:.3} bar drawdown) \
             accounts for. Something other than the conditioning is contributing",
            p.cells,
            p.cumulative_relative * 100.0,
            bound * 100.0,
            p.late_injection_cell_bar,
            p.injector_drawdown_bar
        );

        // The conversion-free observable is what carries the transport claim, and it is small.
        assert!(
            p.inventory_relative < 5e-4,
            "{} cells: CO2 in place disagrees by {:.4}%",
            p.cells,
            p.inventory_relative * 100.0
        );
    }

    // Bounded overall, so a regression that made it worse is still caught.
    let worst = points
        .iter()
        .map(|p| p.cumulative_relative)
        .fold(0.0, f64::max);
    assert!(
        worst < 0.06,
        "cumulative injection differs by {:.2}%, beyond what the conditioning accounts for",
        worst * 100.0
    );
}
