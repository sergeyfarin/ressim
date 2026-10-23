//! The pinned fluids: C0's binary and ternary (`docs/COMPOSITIONAL_VALIDATION.md` §1) and the SPE5
//! hydrocarbon fluid (#45).
//!
//! Every number below was read out of the installed OPM 2026.04 headers, with the owning header
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

use super::specification::{
    Component, EosVariant, FluidSpecError, FluidSpecification, ViscosityModel,
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
