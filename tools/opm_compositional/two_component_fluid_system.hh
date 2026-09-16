// A two-component C1/C10 fluid system for the ResSim compositional plan's N=2 reduction.
//
// This exists because OPM ships `ThreeComponentFluidSystem` and nothing smaller, and because the
// obvious shortcut - run the ternary with z_CO2 = 0 - does not work: PTFlash's Michelsen stability
// test forms z_i/K_i and K_i*z_i, so an exactly zero component makes it trivial and it throws
// "Stability test did not converge". OPM's own source carries the matching TODO ("make sure that
// no mole fraction is smaller than 1e-8?"). See README.md, "Oracle domain limits".
//
// Every property below is delegated to the same `Opm::C1` / `Opm::C10` component classes the
// ternary system uses, and the EOS, parameter cache, viscosity model and zero interaction
// coefficients are OPM's. No value is introduced here. This file is a *declaration* that these
// two OPM components form a system, not a new fluid dataset.

#ifndef RESSIM_TWO_COMPONENT_FLUID_SYSTEM_HH
#define RESSIM_TWO_COMPONENT_FLUID_SYSTEM_HH

#include <opm/material/eos/CubicEOS.hpp>
#include <opm/material/fluidsystems/BaseFluidSystem.hpp>
#include <opm/material/fluidsystems/PTFlashParameterCache.hpp>
#include <opm/material/viscositymodels/LBC.hpp>
#include <opm/material/components/C1.hpp>
#include <opm/material/components/C10.hpp>

#include <stdexcept>
#include <string_view>

namespace ressim {

template <class Scalar>
class TwoComponentFluidSystem
    : public Opm::BaseFluidSystem<Scalar, TwoComponentFluidSystem<Scalar>>
{
public:
    static constexpr int numPhases = 2;
    static constexpr int numComponents = 2;
    static constexpr int numMisciblePhases = 2;
    static constexpr int numMiscibleComponents = 2;
    static constexpr bool waterEnabled = false;

    static constexpr int oilPhaseIdx = 0;
    static constexpr int gasPhaseIdx = 1;
    static constexpr int waterPhaseIdx = -1;

    static constexpr int Comp0Idx = 0;
    static constexpr int Comp1Idx = 1;

    using Comp0 = Opm::C1<Scalar>;
    using Comp1 = Opm::C10<Scalar>;

    template <class ValueType>
    using ParameterCache = Opm::PTFlashParameterCache<ValueType, TwoComponentFluidSystem<Scalar>>;
    using ViscosityModel = Opm::ViscosityModels<Scalar, TwoComponentFluidSystem<Scalar>>;
    using CubicEOS = ::Opm::CubicEOS<Scalar, TwoComponentFluidSystem<Scalar>>;

    static bool phaseIsActive(unsigned phaseIdx)
    {
        return phaseIdx == oilPhaseIdx || phaseIdx == gasPhaseIdx;
    }

#define RESSIM_DISPATCH(fn)                                                                      \
    switch (compIdx) {                                                                           \
    case Comp0Idx: return Comp0::fn();                                                           \
    case Comp1Idx: return Comp1::fn();                                                           \
    default: throw std::runtime_error("Illegal component index for " #fn);                       \
    }

    static Scalar acentricFactor(unsigned compIdx) { RESSIM_DISPATCH(acentricFactor) }
    static Scalar criticalTemperature(unsigned compIdx) { RESSIM_DISPATCH(criticalTemperature) }
    static Scalar criticalPressure(unsigned compIdx) { RESSIM_DISPATCH(criticalPressure) }
    static Scalar criticalVolume(unsigned compIdx) { RESSIM_DISPATCH(criticalVolume) }
    static Scalar molarMass(unsigned compIdx) { RESSIM_DISPATCH(molarMass) }

#undef RESSIM_DISPATCH

    /// Zero for every pair, matching `ThreeComponentFluidSystem::interactionCoefficient`.
    static Scalar interactionCoefficient(unsigned, unsigned) { return 0.0; }

    static std::string_view phaseName(unsigned phaseIdx)
    {
        static const std::string_view name[] = {"o", "g"};
        return name[phaseIdx];
    }

    static std::string_view componentName(unsigned compIdx)
    {
        static const std::string_view name[] = {Comp0::name(), Comp1::name()};
        return name[compIdx];
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

#endif  // RESSIM_TWO_COMPONENT_FLUID_SYSTEM_HH
