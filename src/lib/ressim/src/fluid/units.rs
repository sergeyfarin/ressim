//! Unit conversions at the thermodynamics boundary (compositional plan, C1).
//!
//! ResSim's simulation code works in oil-field units — bar, cP, day, m, mD — as stated at the top
//! of `lib.rs`. The compositional EOS kernel works in SI: Pa, K, mol, m³, kg/mol, J/(mol·K). Both
//! are correct in their own domain and neither is going to move, so the conversion has to live in
//! one named, tested place rather than being sprinkled through the physics.
//!
//! The two that are easy to get wrong, and are therefore spelled out rather than inlined:
//!
//! * **Derivatives carry the factor too.** `d f / d p_bar = 1e5 * d f / d p_Pa`. A property
//!   converted correctly and a derivative converted with the *inverse* factor is a mistake that
//!   finite differences catch late and expensively, because the value looks right.
//! * **Temperature is absolute.** There is no Celsius anywhere past this module. `from_celsius`
//!   exists to convert *input* once, at the edge.
//!
//! Nothing here imports `ReservoirSimulator`, `wasm_bindgen` or any browser type; this module is
//! plain arithmetic over `f64` and is compiled for every target the crate targets.

/// Pascals per bar. Exact by definition of the bar.
pub(crate) const PA_PER_BAR: f64 = 1.0e5;

/// Pascal-seconds per centipoise. Exact by definition of the poise.
pub(crate) const PA_S_PER_CP: f64 = 1.0e-3;

/// Universal gas constant, J/(mol·K). Exact since the 2019 SI redefinition: `R = N_A * k_B`.
pub(crate) const GAS_CONSTANT_J_PER_MOL_K: f64 = 8.314_462_618_153_24;

/// The gas constant OPM uses, and therefore the one baked into
/// `opm/compositional/ptflash_fixtures.json`.
///
/// `/usr/include/opm/material/Constants.hpp` line 47 defines `R = 8.314472` — the CODATA-2006
/// value, superseded by the 2019 redefinition. It differs from [`GAS_CONSTANT_J_PER_MOL_K`] by
/// **+1.128e-6 relative**, which is an order of magnitude *above* the plan's 1e-7 relative
/// admission target for density. Ignoring it would make every density comparison against the
/// fixture fail for a reason that has nothing to do with the EOS.
///
/// ResSim uses the exact SI value in its own physics. This constant exists only so the fixture
/// comparison can convert, and the conversion is exact rather than absorbed into a tolerance,
/// because the dependence is analytic: `A` and `B` contain no `R` at all (`A_i = OmegaA * p_r /
/// T_r^2`, `B_i = OmegaB * p_r / T_r`), so the compressibility factor and the fugacity
/// coefficients are `R`-independent, and only `V_m = Z R T / p` and its reciprocal carry the
/// factor — linearly. See [`opm_density_to_si`].
pub(crate) const OPM_GAS_CONSTANT_J_PER_MOL_K: f64 = 8.314472;

/// Convert a molar or mass density computed with OPM's `R` to one computed with the SI `R`.
///
/// Density is `p / (Z R T)`, so it scales as `1/R`: a larger `R` gives a smaller density. OPM's
/// `R` is the larger of the two, so its densities are the smaller ones and this factor is
/// slightly greater than 1.
pub(crate) fn opm_density_to_si(density_with_opm_r: f64) -> f64 {
    density_with_opm_r * (OPM_GAS_CONSTANT_J_PER_MOL_K / GAS_CONSTANT_J_PER_MOL_K)
}

/// Convert a molar volume computed with OPM's `R` to one computed with the SI `R`.
pub(crate) fn opm_molar_volume_to_si(vm_with_opm_r: f64) -> f64 {
    vm_with_opm_r * (GAS_CONSTANT_J_PER_MOL_K / OPM_GAS_CONSTANT_J_PER_MOL_K)
}

/// Zero Celsius in kelvin.
pub(crate) const KELVIN_AT_ZERO_CELSIUS: f64 = 273.15;

/// Reservoir pressure [bar] to EOS pressure [Pa].
pub(crate) fn bar_to_pa(p_bar: f64) -> f64 {
    p_bar * PA_PER_BAR
}

/// EOS pressure [Pa] to reservoir pressure [bar].
pub(crate) fn pa_to_bar(p_pa: f64) -> f64 {
    p_pa / PA_PER_BAR
}

/// Convert a derivative taken with respect to pressure in Pa into one with respect to bar.
///
/// `d f / d p_bar = (d f / d p_Pa) * (d p_Pa / d p_bar) = 1e5 * d f / d p_Pa`.
///
/// The factor is the *same direction* as [`bar_to_pa`], not its inverse — a derivative with
/// respect to a coarser unit is larger, because one bar is a bigger step than one pascal.
pub(crate) fn dpa_to_dbar(df_dp_pa: f64) -> f64 {
    df_dp_pa * PA_PER_BAR
}

/// Convert a derivative with respect to pressure in bar into one with respect to Pa.
pub(crate) fn dbar_to_dpa(df_dp_bar: f64) -> f64 {
    df_dp_bar / PA_PER_BAR
}

/// EOS viscosity [Pa·s] to reservoir viscosity [cP].
pub(crate) fn pa_s_to_cp(mu_pa_s: f64) -> f64 {
    mu_pa_s / PA_S_PER_CP
}

/// Reservoir viscosity [cP] to EOS viscosity [Pa·s].
pub(crate) fn cp_to_pa_s(mu_cp: f64) -> f64 {
    mu_cp * PA_S_PER_CP
}

/// Degrees Celsius to kelvin, for converting user input exactly once at the edge.
pub(crate) fn from_celsius(t_c: f64) -> f64 {
    t_c + KELVIN_AT_ZERO_CELSIUS
}

/// Molar density [mol/m³] and mean molar mass [kg/mol] to mass density [kg/m³].
///
/// Gravity needs mass density; the EOS produces molar density. Passing the molar one to a
/// gravity term is dimensionally undetectable — both are "density" — and wrong by three orders of
/// magnitude, so the conversion is a named function rather than a multiplication at the call site.
pub(crate) fn mass_density_kg_per_m3(
    molar_density_mol_per_m3: f64,
    mean_molar_mass_kg_per_mol: f64,
) -> f64 {
    molar_density_mol_per_m3 * mean_molar_mass_kg_per_mol
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comp_units_pressure_roundtrip() {
        for p_bar in [1.0, 10.0, 137.5, 500.0, 1e-3] {
            let back = pa_to_bar(bar_to_pa(p_bar));
            assert!(
                (back - p_bar).abs() <= 1e-12 * p_bar.abs().max(1.0),
                "bar -> Pa -> bar drifted: {p_bar} -> {back}"
            );
        }
        assert_eq!(bar_to_pa(1.0), 1.0e5);
        assert_eq!(bar_to_pa(150.0), 1.5e7);
    }

    #[test]
    fn comp_units_viscosity_roundtrip() {
        for mu_cp in [0.01, 0.5, 1.0, 12.75] {
            let back = pa_s_to_cp(cp_to_pa_s(mu_cp));
            assert!((back - mu_cp).abs() <= 1e-12 * mu_cp);
        }
        // 1 cP is 1 mPa·s, the canonical check.
        assert_eq!(cp_to_pa_s(1.0), 1.0e-3);
        // The LBC viscosities in the C0 fixture are ~1e-4 Pa·s for the liquid, i.e. ~0.1 cP.
        assert!((pa_s_to_cp(1.2746e-4) - 0.12746).abs() < 1e-12);
    }

    #[test]
    fn comp_units_temperature_is_absolute() {
        assert_eq!(from_celsius(0.0), 273.15);
        // The C0 fixture's two isotherms.
        assert!((from_celsius(150.0) - 423.15).abs() < 1e-12);
        assert!((from_celsius(60.0) - 333.15).abs() < 1e-12);
    }

    /// The derivative chain factor must scale *up* going from Pa to bar. Getting this inverted is
    /// the specific failure mode this test exists for: the roundtrip alone would still pass.
    #[test]
    fn comp_units_derivative_chain_direction() {
        let df_dp_pa = 3.0e-7;
        let df_dp_bar = dpa_to_dbar(df_dp_pa);
        assert!(
            df_dp_bar > df_dp_pa,
            "d/dp_bar must exceed d/dp_Pa for a positive derivative; got {df_dp_bar} vs {df_dp_pa}"
        );
        assert_eq!(df_dp_bar, 3.0e-2);
        assert!((dbar_to_dpa(df_dp_bar) - df_dp_pa).abs() < 1e-24);
    }

    /// A finite-difference check that the chain factor is consistent with the value conversion.
    /// If `bar_to_pa` and `dpa_to_dbar` ever disagree in direction, this fails even though each
    /// function in isolation looks self-consistent.
    #[test]
    fn comp_units_derivative_matches_finite_difference() {
        // An arbitrary smooth function of pressure in Pa.
        let f = |p_pa: f64| (p_pa / 1.0e7).sqrt();
        let df_dp_pa = |p_pa: f64| 0.5 / (1.0e7 * (p_pa / 1.0e7).sqrt());

        let p_bar = 150.0;
        let h_bar = 1.0e-4;
        let fd_dbar = (f(bar_to_pa(p_bar + h_bar)) - f(bar_to_pa(p_bar - h_bar))) / (2.0 * h_bar);
        let analytic_dbar = dpa_to_dbar(df_dp_pa(bar_to_pa(p_bar)));

        assert!(
            (fd_dbar - analytic_dbar).abs() / analytic_dbar.abs() < 1e-8,
            "chain factor disagrees with FD: analytic {analytic_dbar}, fd {fd_dbar}"
        );
    }

    #[test]
    fn comp_units_mass_density_from_molar() {
        // The C0 fixture's `binary_p150` liquid: 512.75 kg/m³. Its mean molar mass follows from
        // x = [0.4400, 0.5600] over C1 (0.0160) and C10 (0.142) — check the conversion reproduces
        // a mass density of the right order, not the exact fixture value, which C2 owns.
        let molar_density = 6000.0;
        let mean_mw = 0.085;
        assert!((mass_density_kg_per_m3(molar_density, mean_mw) - 510.0).abs() < 1e-9);
    }

    #[test]
    fn comp_units_gas_constant_matches_si_definition() {
        // R = N_A * k_B, both exact since the 2019 SI redefinition.
        let avogadro = 6.022_140_76e23;
        let boltzmann = 1.380_649e-23;
        let r = avogadro * boltzmann;
        assert!(
            (GAS_CONSTANT_J_PER_MOL_K - r).abs() / r < 1e-15,
            "R disagrees with N_A * k_B: {GAS_CONSTANT_J_PER_MOL_K} vs {r}"
        );
    }

    /// The offset between OPM's legacy `R` and the SI one is large enough to break a 1e-7 density
    /// comparison on its own. This test pins its size so the number quoted in
    /// `COMPOSITIONAL_VALIDATION.md` cannot drift away from the code.
    #[test]
    fn comp_units_opm_gas_constant_offset_is_above_the_density_tolerance() {
        let rel = (OPM_GAS_CONSTANT_J_PER_MOL_K - GAS_CONSTANT_J_PER_MOL_K).abs()
            / GAS_CONSTANT_J_PER_MOL_K;
        assert!(
            (rel - 1.128e-6).abs() < 1e-9,
            "the OPM/SI gas-constant offset moved: {rel:e}"
        );
        assert!(
            rel > 1e-7,
            "if this ever drops below the density tolerance the conversion can be retired"
        );
    }

    /// Density scales as `1/R`, so converting one way and back is the identity, and OPM's larger
    /// `R` must yield the *smaller* density — i.e. the correction factor is above one.
    #[test]
    fn comp_units_opm_density_conversion_is_invertible_and_signed_correctly() {
        let rho_opm = 543.54;
        let rho_si = opm_density_to_si(rho_opm);
        assert!(
            rho_si > rho_opm,
            "OPM's larger R gives a smaller density, so the SI value must be larger"
        );
        // Round-trip through the molar-volume form, which carries the reciprocal factor.
        let vm_opm = 1.0 / rho_opm;
        let vm_si = opm_molar_volume_to_si(vm_opm);
        assert!(
            (1.0 / vm_si - rho_si).abs() / rho_si < 1e-15,
            "the volume and density conversions must be exact reciprocals"
        );
    }
}
