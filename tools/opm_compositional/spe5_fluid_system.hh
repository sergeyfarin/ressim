// The six hydrocarbon components of OPM's `Spe5FluidSystem`, as a PTFlash-capable system (#45).
//
// `Opm::Spe5FluidSystem` is the source of the SPE5 fluid data: components C1, C3, C6, C10, C15 and
// C20 (plus H2O), their molar masses, critical temperatures and pressures, acentric factors, the
// critical volumes it builds from per-component Z_c, and the nonzero interaction coefficients
// C1-C15/C20 = 0.05 and C3-C15/C20 = 0.005. It cannot be flashed as it stands: it runs its own
// `Spe5ParameterCache` rather than `PTFlashParameterCache`, carries a water component the ResSim
// compositional model does not have, and has no `criticalVolume()` for LBC.
//
// So this class does what `two_component_fluid_system.hh` does for C1/C10: it declares a system
// and delegates every property to the header, offset by one to skip `H2OIdx`. No value is
// introduced here. The only conversion is the critical volume's unit, m^3/mol in the header to
// the m^3/kmol LBC consumes, which is what `Opm::C1::criticalVolume()` already reports.
//
// The EOS is OPM's `PRCORR`, which is the `useSpe5` relation: `PengRobinsonParamsMixture`'s SPE5
// path switches f(w) to the 1978 quartic above w = 0.49, and `PRParams::calcOmegaA(modified=true)`
// is that same switch. C15 (w = 0.65) and C20 (w = 0.85) take the quartic; nothing sits at the
// boundary, so the header's `< 0.49` and PRParams' `<= 0.49` select the same branch for every
// component. `docs/COMPOSITIONAL_VALIDATION.md` records the one place the two disagree: the SPE5
// mixture truncates Omega_A and Omega_B to 0.4572355 and 0.0777961, PRParams does not.
//
// The viscosity is LBC, as for the other two systems. The header's own constant phase viscosities
// (0.0170 cP gas, 0.208 cP oil) are not part of this system and are not pinned.

#ifndef RESSIM_SPE5_FLUID_SYSTEM_HH
#define RESSIM_SPE5_FLUID_SYSTEM_HH

#include <opm/material/eos/CubicEOS.hpp>
#include <opm/material/fluidsystems/BaseFluidSystem.hpp>
#include <opm/material/fluidsystems/PTFlashParameterCache.hpp>
#include <opm/material/fluidsystems/Spe5FluidSystem.hpp>
#include <opm/material/viscositymodels/LBC.hpp>

#include <stdexcept>
#include <string_view>

namespace ressim {

template <class Scalar>
class Spe5HydrocarbonFluidSystem
    : public Opm::BaseFluidSystem<Scalar, Spe5HydrocarbonFluidSystem<Scalar>>
{
    using Spe5 = Opm::Spe5FluidSystem<Scalar>;

    // Header index of hydrocarbon component `compIdx`. `H2OIdx` is 0 and the six hydrocarbons
    // follow it in order, so the offset is the whole mapping.
    static unsigned spe5Index(unsigned compIdx)
    {
        if (compIdx >= numComponents) {
            throw std::runtime_error("Illegal component index for Spe5HydrocarbonFluidSystem");
        }
        return compIdx + 1;
    }

public:
    static constexpr int numPhases = 2;
    static constexpr int numComponents = 6;
    static constexpr int numMisciblePhases = 2;
    static constexpr int numMiscibleComponents = 6;
    static constexpr bool waterEnabled = false;

    static constexpr int oilPhaseIdx = 0;
    static constexpr int gasPhaseIdx = 1;
    static constexpr int waterPhaseIdx = -1;

    template <class ValueType>
    using ParameterCache =
        Opm::PTFlashParameterCache<ValueType, Spe5HydrocarbonFluidSystem<Scalar>>;
    using ViscosityModel = Opm::ViscosityModels<Scalar, Spe5HydrocarbonFluidSystem<Scalar>>;
    using CubicEOS = ::Opm::CubicEOS<Scalar, Spe5HydrocarbonFluidSystem<Scalar>>;

    static bool phaseIsActive(unsigned phaseIdx)
    {
        return phaseIdx == oilPhaseIdx || phaseIdx == gasPhaseIdx;
    }

    static Scalar acentricFactor(unsigned compIdx)
    {
        return Spe5::acentricFactor(spe5Index(compIdx));
    }
    static Scalar criticalTemperature(unsigned compIdx)
    {
        return Spe5::criticalTemperature(spe5Index(compIdx));
    }
    static Scalar criticalPressure(unsigned compIdx)
    {
        return Spe5::criticalPressure(spe5Index(compIdx));
    }
    /// [m^3/kmol]. The header's `criticalMolarVolume` is m^3/mol.
    static Scalar criticalVolume(unsigned compIdx)
    {
        return Spe5::criticalMolarVolume(spe5Index(compIdx)) * 1e3;
    }
    static Scalar molarMass(unsigned compIdx) { return Spe5::molarMass(spe5Index(compIdx)); }

    static Scalar interactionCoefficient(unsigned comp1Idx, unsigned comp2Idx)
    {
        return Spe5::interactionCoefficient(spe5Index(comp1Idx), spe5Index(comp2Idx));
    }

    static std::string_view phaseName(unsigned phaseIdx)
    {
        static const std::string_view name[] = {"o", "g"};
        return name[phaseIdx];
    }

    static std::string_view componentName(unsigned compIdx)
    {
        return Spe5::componentName(spe5Index(compIdx));
    }

    template <class FluidState, class LhsEval = typename FluidState::ValueType,
              class ParamCacheEval = LhsEval>
    static LhsEval density(const FluidState& fluidState,
                           const ParameterCache<ParamCacheEval>& paramCache, unsigned phaseIdx)
    {
        return Opm::decay<LhsEval>(fluidState.averageMolarMass(phaseIdx)
                                   / paramCache.molarVolume(phaseIdx));
    }

    template <class FluidState, class LhsEval = typename FluidState::ValueType,
              class ParamCacheEval = LhsEval>
    static LhsEval viscosity(const FluidState& fluidState,
                             const ParameterCache<ParamCacheEval>& paramCache, unsigned phaseIdx)
    {
        return Opm::decay<LhsEval>(ViscosityModel::LBC(fluidState, paramCache, phaseIdx));
    }

    template <class FluidState, class LhsEval = typename FluidState::ValueType,
              class ParamCacheEval = LhsEval>
    static LhsEval fugacityCoefficient(const FluidState& fluidState,
                                       const ParameterCache<ParamCacheEval>& paramCache,
                                       unsigned phaseIdx, unsigned compIdx)
    {
        return Opm::decay<LhsEval>(
            CubicEOS::computeFugacityCoefficient(fluidState, paramCache, phaseIdx, compIdx));
    }

    static bool isCompressible(unsigned) { return true; }
    static bool isIdealMixture(unsigned) { return false; }
    static bool isLiquid(unsigned phaseIdx) { return phaseIdx == oilPhaseIdx; }
    static bool isIdealGas(unsigned phaseIdx) { return phaseIdx == gasPhaseIdx; }
};

}  // namespace ressim

#endif  // RESSIM_SPE5_FLUID_SYSTEM_HH
