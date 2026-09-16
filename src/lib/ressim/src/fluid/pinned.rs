//! The two fluids V1 supports, as pinned by C0 (`docs/COMPOSITIONAL_VALIDATION.md` §1).
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
//! The interaction matrix is all zeros for both, because
//! `ThreeComponentFluidSystem::interactionCoefficient()` returns `0.0` unconditionally. That is
//! that source's stated modelling choice and is citable as such. It is **not** a default: a fluid
//! with nonzero binary interactions needs its own source before it can be added here.

use super::specification::{
    Component, EosVariant, FluidSpecError, FluidSpecification, ViscosityModel,
};

/// The reservoir temperature both pinned fluids are specified at: 150 °C.
///
/// Matches the primary isotherm of the C0 fixture. The fixture's second isotherm, 333.15 K, exists
/// to prove temperature actually reaches the EOS and is not a hard-coded constant; it is a test
/// condition, not a second pinned fluid.
pub const PINNED_RESERVOIR_TEMPERATURE_K: f64 = 423.15;

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
        PINNED_RESERVOIR_TEMPERATURE_K,
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
        PINNED_RESERVOIR_TEMPERATURE_K,
        None,
    )
}
