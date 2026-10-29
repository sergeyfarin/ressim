//! The pinned fluids: C0's binary and ternary (`docs/COMPOSITIONAL_VALIDATION.md` §1) and the SPE5
//! hydrocarbon fluid (#45), plus the published nine-component SPE3 fluid (#51).
//!
//! The C0 and SPE5 numbers were read out of the installed OPM 2026.04 headers, with the owning header
//! named next to it. None was chosen, fitted or rounded here. The reference fixture in
//! `opm/compositional/ptflash_fixtures.json` was generated from the same headers by
//! `tools/opm_compositional/`, so these constants and that fixture describe one fluid — if they
//! ever disagree, one of them was edited by hand and the other is right.
//!
//! [`ternary`] is OPM's `ThreeComponentFluidSystem`. [`binary`] is its C1/C10 subset, which OPM
//! does not ship as a system; the plan uses it as the N=2 reduction, and the harness declares the
//! matching C++ system for the same reason.
//!
//! The interaction matrix is all zeros for those two, because
//! `ThreeComponentFluidSystem::interactionCoefficient()` returns `0.0` unconditionally. That is
//! that source's stated modelling choice and is citable as such. It is **not** a default: a fluid
//! with nonzero binary interactions needs its own source before it can be added here.
//!
//! [`spe5`] is that case. `Opm::Spe5FluidSystem` is its source, for the components and for the
//! nonzero interactions alike; the harness flashes it through
//! `tools/opm_compositional/spe5_fluid_system.hh`, which delegates every property to that header.
//!
//! Temperature belongs to each fluid, not to the crate: every constructor below names its own.
//! [`spe3`] instead cites Juell, Whitson & Hoda's published characterization, with its independent
//! flash and depletion oracle in `opm/compositional/spe3/`.

use super::specification::{
    Component, EosVariant, FluidSpecError, FluidSpecification, SurfaceConditions, ViscosityModel,
};

use super::units::OPM_GAS_CONSTANT_J_PER_MOL_K;

/// The temperature [`binary`] and [`ternary`] are specified at: 150 °C.
///
/// Matches the primary isotherm of the C0 fixture. The fixture's second isotherm, 333.15 K, exists
/// to prove temperature actually reaches the EOS and is not a hard-coded constant; it is a test
/// condition, not a second pinned fluid.
const C0_RESERVOIR_TEMPERATURE_K: f64 = 423.15;

/// The SPE5 reservoir temperature: 160 °F.
///
/// Not in `Spe5FluidSystem.hpp`, which takes temperature from its caller. The value is the
/// reservoir temperature of Killough, J.E. & Kossack, C.A. (1987), "Fifth Comparative Solution
/// Project: Evaluation of Miscible Flood Simulators", SPE 16000, Ninth SPE Symposium on Reservoir
/// Simulation, San Antonio — the paper the header itself cites. Written as the Fahrenheit
/// conversion, the same way the header writes its Rankine critical temperatures, so it stays
/// traceable to that source; the harness uses the same expression.
pub const SPE5_RESERVOIR_TEMPERATURE_K: f64 = (160.0 + 459.67) * 5.0 / 9.0;

/// Methane. `/usr/include/opm/material/components/C1.hpp`.
fn c1() -> Component {
    Component {
        id: "C1".to_string(),
        molar_mass_kg_per_mol: 0.0160,
        critical_temperature_k: 190.6,
        critical_pressure_pa: 4.60e6,
        critical_volume_m3_per_kmol: 9.863e-2,
        acentric_factor: 0.011,
    }
}

/// SPE3's original reservoir composition in the nine-component Juell/Whitson/Hoda ordering.
pub const SPE3_COMPOSITION: [f64; 9] = [
    0.0121, 0.0194, 0.6599, 0.0869, 0.0591, 0.0967, 0.0472, 0.0153, 0.0034,
];

/// SPE3 at 200 °F, using the nine-component characterization of Juell, Whitson & Hoda,
/// SPE 121252 (2009), Tables 3–5 (ARCO-derived, including their published volume shifts).
/// See `docs/SPE3_VALIDATION.md` for provenance, EOS convention and the independent oracle.
pub fn spe3() -> Result<FluidSpecification, FluidSpecError> {
    // id, MW [g/mol], Tc [K], Pc [bara], Zc, acentric factor, dimensionless shift.
    let rows = [
        ("CO2", 44.01, 304.21, 73.82, 0.27400, 0.225, -0.00089),
        ("N2", 28.02, 126.27, 33.90, 0.29000, 0.040, -0.16453),
        ("C1", 16.04, 186.61, 46.20, 0.28800, 0.013, -0.17817),
        ("C2", 30.07, 305.33, 48.80, 0.28500, 0.098, -0.06456),
        ("C3", 44.10, 369.85, 42.50, 0.28100, 0.152, -0.06439),
        ("C4-6", 67.28, 396.22, 34.35, 0.27228, 0.234, -0.18129),
        ("C7+1", 110.90, 572.50, 25.94, 0.26444, 0.332, 0.12080),
        ("C7+2", 170.90, 630.22, 16.92, 0.25140, 0.495, 0.23442),
        ("C7+3", 282.10, 862.61, 8.61, 0.22436, 0.833, 0.54479),
    ];
    let components = rows
        .iter()
        .map(|&(id, mw, tc, pc, zc, ac, _)| Component {
            id: id.to_string(),
            molar_mass_kg_per_mol: mw * 1e-3,
            critical_temperature_k: tc,
            critical_pressure_pa: pc * 1e5,
            critical_volume_m3_per_kmol: zc * super::units::GAS_CONSTANT_J_PER_MOL_K * tc
                / (pc * 1e5)
                * 1e3,
            acentric_factor: ac,
        })
        .collect();
    // Table 4, lower triangle. Zeros are stated values, not an assumed default.
    let lower: [&[f64]; 9] = [
        &[],
        &[0.0],
        &[0.13, 0.036],
        &[0.135, 0.05, 0.0],
        &[0.1277, 0.08, 0.0, 0.0],
        &[0.1, 0.1002, 0.09281, 0.0, 0.0],
        &[0.1, 0.1, 0.0, 0.00385, 0.00385, 0.0],
        &[0.1, 0.1, 0.0, 0.00630, 0.00630, 0.0, 0.0],
        &[0.1, 0.1, 0.1392, 0.006, 0.006, 0.0, 0.0, 0.0],
    ];
    let mut interaction = zero_interaction(9);
    for (i, row) in lower.iter().enumerate() {
        for (j, &k) in row.iter().enumerate() {
            interaction[i][j] = k;
            interaction[j][i] = k;
        }
    }
    let stages = spe3_separator_stages(false);
    FluidSpecification::new(
        components,
        interaction,
        EosVariant::PengRobinson,
        ViscosityModel::LohrenzBrayClark,
        (200.0 + 459.67) * 5.0 / 9.0,
        Some(stages[2]),
    )?
    .with_volume_shifts(rows.iter().map(|r| r.6).collect())?
    .with_separator_stages(stages)
}

/// Kenyon & Behie (1987), Table 3: primary (815 or 315 psia), second stage (65 psia),
/// stock tank (14.7 psia); 80/80/60 °F. Switch primary pressure below 2500 psia reservoir pressure.
pub fn spe3_separator_stages(low_pressure_primary: bool) -> Vec<SurfaceConditions> {
    [
        (if low_pressure_primary { 315.0 } else { 815.0 }, 80.0),
        (65.0, 80.0),
        (14.7, 60.0),
    ]
    .iter()
    .map(|&(p, t)| SurfaceConditions {
        pressure_pa: p * 6894.757293168,
        temperature_k: (t + 459.67) * 5.0 / 9.0,
    })
    .collect()
}

/// n-Decane. `/usr/include/opm/material/components/C10.hpp`.
fn c10() -> Component {
    Component {
        id: "C10".to_string(),
        molar_mass_kg_per_mol: 0.142,
        critical_temperature_k: 617.7,
        critical_pressure_pa: 2.10e6,
        critical_volume_m3_per_kmol: 6.098e-1,
        acentric_factor: 0.488,
    }
}

/// Carbon dioxide. `/usr/include/opm/material/components/SimpleCO2.hpp`.
///
/// A PR-EOS component of a hydrocarbon system at fixed temperature. This is **not** aqueous CO2:
/// brine, dissolution and storage applications are out of V1's scope and would need a different
/// thermodynamic model, not a different parameter.
fn co2() -> Component {
    Component {
        id: "CO2".to_string(),
        molar_mass_kg_per_mol: 44e-3,
        // Written as `273.15 + 30.95` in the header; reproduced in that form rather than as a
        // pre-computed 304.1, so the value stays traceable to the source expression.
        critical_temperature_k: 273.15 + 30.95,
        critical_pressure_pa: 73.8e5,
        critical_volume_m3_per_kmol: 9.412e-2,
        acentric_factor: 0.224,
    }
}

/// psia to Pa, exactly as `Spe5FluidSystem.hpp` writes it.
const SPE5_PA_PER_PSI: f64 = 6894.7573;

/// One SPE5 hydrocarbon, transcribed from `Spe5FluidSystem.hpp`.
///
/// The header gives critical temperature in Rankine (`x*5/9`), critical pressure in psia, and the
/// critical volume as `Z_c R T_c / p_c` in m³/mol with OPM's own `R`. Each is reproduced as that
/// expression, in the header's evaluation order, so the result is the header's double and not a
/// rounded copy of it; the critical volume is then scaled to the m³/kmol this crate stores.
fn spe5_component(
    id: &str,
    molar_mass_kg_per_mol: f64,
    critical_temperature_rankine: f64,
    critical_pressure_psia: f64,
    critical_z: f64,
    acentric_factor: f64,
) -> Component {
    let critical_temperature_k = critical_temperature_rankine * 5.0 / 9.0;
    let critical_pressure_pa = critical_pressure_psia * SPE5_PA_PER_PSI;
    Component {
        id: id.to_string(),
        molar_mass_kg_per_mol,
        critical_temperature_k,
        critical_pressure_pa,
        critical_volume_m3_per_kmol: critical_z
            * OPM_GAS_CONSTANT_J_PER_MOL_K
            * critical_temperature_k
            / critical_pressure_pa
            * 1e3,
        acentric_factor,
    }
}

fn zero_interaction(n: usize) -> Vec<Vec<f64>> {
    vec![vec![0.0; n]; n]
}

/// C1/C10 binary, the N=2 fluid. Component order is `[C1, C10]`.
///
/// Surface conditions are `None`: C0 did not pin them, and C5 owes that decision. See
/// `docs/COMPOSITIONAL_VALIDATION.md` §6.
pub fn binary() -> Result<FluidSpecification, FluidSpecError> {
    FluidSpecification::new(
        vec![c1(), c10()],
        zero_interaction(2),
        EosVariant::PengRobinson,
        ViscosityModel::LohrenzBrayClark,
        C0_RESERVOIR_TEMPERATURE_K,
        None,
    )
}

/// CO2/C1/C10 ternary, the N=3 fluid. Component order is `[CO2, C1, C10]`.
///
/// The order matches `ThreeComponentFluidSystem`'s `Comp0Idx..Comp2Idx` and therefore the fixture,
/// so a fixture state's `z`, `x` and `y` arrays index straight into this specification.
pub fn ternary() -> Result<FluidSpecification, FluidSpecError> {
    FluidSpecification::new(
        vec![co2(), c1(), c10()],
        zero_interaction(3),
        EosVariant::PengRobinson,
        ViscosityModel::LohrenzBrayClark,
        C0_RESERVOIR_TEMPERATURE_K,
        None,
    )
}

/// The SPE5 hydrocarbon fluid: `[C1, C3, C6, C10, C15, C20]`. `Opm::Spe5FluidSystem`.
///
/// The order is the header's `C1Idx..C20Idx` with `H2OIdx` removed — water is not part of the
/// compositional model — so a fixture state's arrays index straight into this specification.
///
/// The interactions are the header's `interactionCoefficient`: `k(C1, C15) = k(C1, C20) = 0.05`,
/// `k(C3, C15) = k(C3, C20) = 0.005`, zero otherwise. The EOS is
/// [`EosVariant::PengRobinsonCorrected`], the header's `useSpe5` relation: C15 and C20 have
/// `w > 0.49` and take the 1978 `f(w)`. The header's constant phase viscosities are not carried:
/// viscosity is LBC, as for every pinned fluid, and the fixture's viscosities are LBC's.
///
/// Surface conditions are `None`, as for the C0 fluids; a case that reports surface volumes
/// attaches its own `STCOND`.
pub fn spe5() -> Result<FluidSpecification, FluidSpecError> {
    let components = vec![
        spe5_component("C1", 16.04e-3, 343.0, 667.8, 0.290, 0.0130),
        spe5_component("C3", 44.10e-3, 665.7, 616.3, 0.277, 0.1524),
        spe5_component("C6", 86.18e-3, 913.4, 436.9, 0.264, 0.3007),
        spe5_component("C10", 142.29e-3, 1111.8, 304.0, 0.257, 0.4885),
        spe5_component("C15", 206.00e-3, 1270.0, 200.0, 0.245, 0.6500),
        spe5_component("C20", 282.00e-3, 1380.0, 162.0, 0.235, 0.8500),
    ];
    let n = components.len();
    let mut interaction = zero_interaction(n);
    // Indices into the order above: C1 = 0, C3 = 1, C15 = 4, C20 = 5.
    for (light, k) in [(0usize, 0.05), (1usize, 0.005)] {
        for heavy in [4usize, 5usize] {
            interaction[light][heavy] = k;
            interaction[heavy][light] = k;
        }
    }
    FluidSpecification::new(
        components,
        interaction,
        EosVariant::PengRobinsonCorrected,
        ViscosityModel::LohrenzBrayClark,
        SPE5_RESERVOIR_TEMPERATURE_K,
        None,
    )
}
