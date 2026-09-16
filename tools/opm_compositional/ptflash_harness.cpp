// PTFlash fixture generator for the ResSim compositional plan, task C0.
//
// OPM's material library is header-only, so this compiles directly against the installed
// libopm-common-dev / libopm-simulators-dev 2026.04 headers; only Dune's exception vtable and
// libfmt are linked. See README.md for the exact build command, package provenance, the oracle's
// measured domain limits, and why its output is an oracle rather than a convenience.
//
// The ternary fluid is OPM's own `ThreeComponentFluidSystem` (CO2 / C1 / C10), unmodified. The
// binary is `ressim::TwoComponentFluidSystem`, which declares OPM's C1 and C10 components as a
// two-component system and delegates every property to them. This program does not define,
// override or tune a single fluid property; every number it prints comes out of OPM's PR EOS,
// PTFlash and LBC code paths.
//
// PTFlash::solve reconstructs derivatives of the converged equilibrium by implicit
// differentiation, so its outer fluid state must carry an AD type - a plain double does not
// compile. That is not an obstacle, it is the reason this harness is also C4's oracle: seeding
// the AD slots with ResSim's own primary variables makes OPM hand back dx/du, dy/du and dL/du
// for exactly the u = [p, z_0 .. z_(N-2)] this plan specifies, dependent z_(N-1) included.
//
// Output is one JSON object on stdout. Conventions, chosen to be unambiguous at the Rust boundary:
//   - `L_liquid` is OPM's liquid mole fraction, reported exactly as OPM computed it.
//   - `beta_vapour` is the VAPOUR mole fraction, 1 - L, computed here so the Rust side never has
//     to guess which convention a field follows.
//   - pressures in Pa, temperature in K, molar volume m^3/mol, molar density mol/m^3,
//     mass density kg/m^3, viscosity Pa*s. No bar, no cP, no Celsius anywhere in this file.
//
// SPDX note: this file is ResSim's own code. It includes GPL-2+ OPM headers and is distributed
// under the same terms as the rest of this repository's OPM tooling; the OPM copyright notices
// remain in the headers it includes.

// OPM's EOS headers call assert() without including <cassert>; do it for them. The asserts stay
// enabled on purpose (no -DNDEBUG): an oracle that silently emits an unphysical molar volume is
// worse than one that aborts.
#include <cassert>

#include <opm/material/fluidsystems/ThreeComponentFluidSystem.hh>
#include <opm/material/fluidstates/CompositionalFluidState.hpp>
#include <opm/material/constraintsolvers/PTFlash.hpp>
#include <opm/material/viscositymodels/LBC.hpp>
#include <opm/material/densead/Evaluation.hpp>
#include <opm/material/densead/Math.hpp>
#include <opm/input/eclipse/EclipseState/Compositional/CompositionalConfig.hpp>

#include "two_component_fluid_system.hh"

#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <string>
#include <vector>

using Scalar = double;
using EOSType = Opm::CompositionalConfig::EOSType;

// The plan requires enough precision to resolve a 1e-8 absolute / 1e-7 relative admission target.
// 17 significant digits round-trips an IEEE double exactly, so the fixture never limits the test.
static std::string num(Scalar v)
{
    std::ostringstream os;
    os << std::setprecision(17) << v;
    return os.str();
}

template <int NC>
struct Case {
    std::string id;
    Scalar p_pa;
    Scalar t_k;
    std::array<Scalar, NC> z;
};

// ResSim's primary variables, in ResSim's order: u = [p, z_0, ..., z_(N-2)]. The AD slot count is
// therefore N, not N+1 - z_(N-1) is dependent and is seeded with derivative -1 in every
// composition slot rather than given one of its own.
template <int NC>
struct Result {
    // A state the OPM oracle itself cannot resolve is recorded, not dropped. The plan is explicit
    // that a sweep with failures quietly excluded is not a validated domain, and a Rust
    // implementation must be allowed to see exactly where its reference gave up.
    std::string status = "ok";
    std::string error;
    std::string method_used;

    bool single_phase = false;
    Scalar L = 0.0;
    std::array<Scalar, NC> x{}, y{};
    std::array<Scalar, 2> vm{}, rho_molar{}, rho_mass{}, z_factor{}, mu{};
    std::array<std::array<Scalar, NC>, 2> phi{};
    std::array<Scalar, 2> eos_A{}, eos_B{};

    std::array<Scalar, NC> dL{};
    std::array<std::array<Scalar, NC>, NC> dx{}, dy{};
    std::array<std::array<Scalar, NC>, 2> drho_molar{}, drho_mass{}, dmu{};
};

template <class FluidSystem>
struct Runner {
    static constexpr int NC = FluidSystem::numComponents;
    static constexpr int NVAR = NC;  // p, then z_0 .. z_(NC-2)
    static constexpr int VAR_P = 0;
    static constexpr int VAR_Z0 = 1;
    static constexpr int OIL = FluidSystem::oilPhaseIdx;
    static constexpr int GAS = FluidSystem::gasPhaseIdx;

    using Eval = Opm::DenseAd::Evaluation<Scalar, NVAR>;
    using FluidState = Opm::CompositionalFluidState<Eval, FluidSystem>;
    using Flash = Opm::PTFlash<Scalar, FluidSystem>;

    static Result<NC> run(const Case<NC>& c, const EOSType eos_type, const Scalar tol,
                          const std::vector<std::string>& methods)
    {
        Result<NC> r;
        FluidState fs;

        for (size_t attempt = 0; attempt < methods.size(); ++attempt) {
            fs = FluidState{};
            seed(fs, c);
            r.method_used = methods[attempt];
            try {
                r.single_phase = Flash::solve(fs, methods[attempt], tol, eos_type, 0);
                r.status = "ok";
                r.error.clear();
                break;
            } catch (const std::exception& e) {
                r.status = "oracle_failure";
                r.error = e.what();
            }
        }
        if (r.status != "ok") {
            return r;
        }

        r.L = fs.L().value();
        for (int v = 0; v < NVAR; ++v) {
            r.dL[v] = fs.L().derivative(v);
        }
        for (int i = 0; i < NC; ++i) {
            r.x[i] = fs.moleFraction(OIL, i).value();
            r.y[i] = fs.moleFraction(GAS, i).value();
            for (int v = 0; v < NVAR; ++v) {
                r.dx[i][v] = fs.moleFraction(OIL, i).derivative(v);
                r.dy[i][v] = fs.moleFraction(GAS, i).derivative(v);
            }
        }

        // Properties are evaluated on both EOS branches regardless of the phase state. In a
        // single-phase result one branch is a *label*, not a coexisting phase: C2 needs exactly
        // that case to prove two algebraic roots are not two stable phases. `present_phase` in the
        // JSON says which branch is physical; consumers must not read the other as material.
        const Eval p = fs.pressure(OIL);
        typename FluidSystem::template ParameterCache<Eval> cache(eos_type);
        try {
            for (int ph = 0; ph < 2; ++ph) {
                cache.updatePhase(fs, ph);
                const Eval vm = cache.molarVolume(ph);
                const Eval rho_molar = 1.0 / vm;
                const Eval rho_mass = fs.averageMolarMass(ph) / vm;
                const Eval z_factor = p * vm / (Opm::Constants<Scalar>::R * c.t_k);
                r.vm[ph] = vm.value();
                r.rho_molar[ph] = rho_molar.value();
                r.rho_mass[ph] = rho_mass.value();
                r.z_factor[ph] = z_factor.value();
                r.eos_A[ph] = cache.A(ph).value();
                r.eos_B[ph] = cache.B(ph).value();
                for (int i = 0; i < NC; ++i) {
                    const Eval phi = FluidSystem::fugacityCoefficient(fs, cache, ph, i);
                    r.phi[ph][i] = phi.value();
                    fs.setFugacityCoefficient(ph, i, phi);
                }
                fs.setDensity(ph, rho_mass);
                // LBC reads Z off the fluid state and nothing in PTFlash writes it, so without
                // this line every viscosity comes back NaN. Z comes from this phase's own molar
                // volume - the same quantity EOS root selection produced, not a second
                // correlation.
                fs.setCompressFactor(ph, z_factor);
                const Eval mu = FluidSystem::viscosity(fs, cache, ph);
                r.mu[ph] = mu.value();
                for (int v = 0; v < NVAR; ++v) {
                    r.drho_molar[ph][v] = rho_molar.derivative(v);
                    r.drho_mass[ph][v] = rho_mass.derivative(v);
                    r.dmu[ph][v] = mu.derivative(v);
                }
            }
        } catch (const std::exception& e) {
            r.status = "property_failure";
            r.error = e.what();
        }
        return r;
    }

private:
    static void seed(FluidState& fs, const Case<NC>& c)
    {
        fs.setTemperature(Eval::createConstant(c.t_k));  // fixed-T model: T carries no AD slot

        // Both phase pressures are the same object because V1 has zero hydrocarbon capillary
        // pressure. If that ever changes, this is one of the places that must.
        const Eval p = Eval::createVariable(c.p_pa, VAR_P);
        fs.setPressure(OIL, p);
        fs.setPressure(GAS, p);

        // z_0 .. z_(NC-2) each get a slot. z_(NC-1) is dependent: dz_(NC-1)/dz_k = -1 for every
        // independent k, which is precisely the invariant C4 must verify. Seeding it here rather
        // than asserting it later means the oracle cannot accidentally agree with a Rust
        // implementation that got the dependent component wrong.
        for (int i = 0; i + 1 < NC; ++i) {
            fs.setMoleFraction(i, Eval::createVariable(c.z[i], VAR_Z0 + i));
        }
        Eval z_last = Eval::createConstant(c.z[NC - 1]);
        for (int k = 0; k + 1 < NC; ++k) {
            z_last.setDerivative(VAR_Z0 + k, -1.0);
        }
        fs.setMoleFraction(NC - 1, z_last);

        // L outside (0,1) is PTFlash's own signal to run the stability test rather than trust a
        // warm start. Every fixture is generated cold on purpose: one that depended on a previous
        // cell's K values would not be reproducible, and C3 has to prove its converged state does
        // not depend on the guess it started from.
        //
        // "Cold" here means the Wilson correlation, not zero. PTFlash's Michelsen test divides by
        // the incoming K on its liquid branch and multiplies by it on its vapour branch, so K = 0
        // is not a neutral starting point - it is a degenerate one, and every state fails with
        // "Stability test did not converge". The correlation below is `wilsonK_` from the same
        // header, reproduced because PTFlash never seeds it itself; it expects the caller to.
        fs.setLvalue(Eval::createConstant(-1.0));
        for (int i = 0; i < NC; ++i) {
            const Scalar acf = FluidSystem::acentricFactor(i);
            const Scalar tc = FluidSystem::criticalTemperature(i);
            const Scalar pc = FluidSystem::criticalPressure(i);
            const Scalar k = std::exp(5.3727 * (1.0 + acf) * (1.0 - tc / c.t_k)) * (pc / c.p_pa);
            fs.setKvalue(i, Eval::createConstant(k));
        }
    }
};

template <size_t N>
static void emit_array(std::ostream& os, const std::array<Scalar, N>& a)
{
    os << "[";
    for (size_t i = 0; i < N; ++i) {
        os << (i ? ", " : "") << num(a[i]);
    }
    os << "]";
}

template <size_t ROWS, size_t COLS>
static void emit_matrix(std::ostream& os, const std::array<std::array<Scalar, COLS>, ROWS>& m)
{
    os << "[";
    for (size_t i = 0; i < ROWS; ++i) {
        os << (i ? ", " : "");
        emit_array(os, m[i]);
    }
    os << "]";
}

template <class FluidSystem>
static void emit_system(std::ostream& os, const std::string& system_name,
                        const std::vector<Case<FluidSystem::numComponents>>& cases,
                        const EOSType eos_type, const Scalar tol,
                        const std::vector<std::string>& methods, bool last)
{
    constexpr int NC = FluidSystem::numComponents;
    os << "    {\n";
    os << "      \"system\": \"" << system_name << "\",\n";
    os << "      \"num_components\": " << NC << ",\n";

    os << "      \"components\": [\n";
    for (int i = 0; i < NC; ++i) {
        os << "        {\"index\": " << i << ", \"name\": \"" << FluidSystem::componentName(i)
           << "\", \"molar_mass_kg_per_mol\": " << num(FluidSystem::molarMass(i))
           << ", \"critical_temperature_k\": " << num(FluidSystem::criticalTemperature(i))
           << ", \"critical_pressure_pa\": " << num(FluidSystem::criticalPressure(i))
           << ", \"critical_volume_m3_per_kmol\": " << num(FluidSystem::criticalVolume(i))
           << ", \"acentric_factor\": " << num(FluidSystem::acentricFactor(i)) << "}"
           << (i + 1 < NC ? "," : "") << "\n";
    }
    os << "      ],\n";

    os << "      \"binary_interaction\": [";
    for (int i = 0; i < NC; ++i) {
        os << (i ? ", " : "") << "[";
        for (int j = 0; j < NC; ++j) {
            os << (j ? ", " : "") << num(FluidSystem::interactionCoefficient(i, j));
        }
        os << "]";
    }
    os << "],\n";

    os << "      \"derivative_variables\": [\"pressure_pa\"";
    for (int k = 0; k + 1 < NC; ++k) {
        os << ", \"z_" << k << "\"";
    }
    os << "],\n";

    os << "      \"states\": [\n";
    for (size_t n = 0; n < cases.size(); ++n) {
        const auto& c = cases[n];
        const auto r = Runner<FluidSystem>::run(c, eos_type, tol, methods);
        const Scalar beta = 1.0 - r.L;
        // OPM's Li labelling returns L = 1 for a single liquid and L = 0 for a single vapour.
        const char* present = (r.status != "ok")      ? "unresolved"
                              : !r.single_phase       ? "two_phase"
                              : (r.L > 0.5)           ? "liquid"
                                                      : "vapour";

        os << "        {\n";
        os << "          \"id\": \"" << c.id << "\",\n";
        os << "          \"status\": \"" << r.status << "\",\n";
        os << "          \"method_used\": \"" << r.method_used << "\",\n";
        os << "          \"pressure_pa\": " << num(c.p_pa) << ",\n";
        os << "          \"temperature_k\": " << num(c.t_k) << ",\n";
        os << "          \"z\": ";
        emit_array(os, c.z);
        os << ",\n";
        os << "          \"present_phase\": \"" << present << "\"";
        if (r.status != "ok") {
            os << ",\n          \"error\": \"" << r.error << "\"\n";
            os << "        }" << (n + 1 < cases.size() ? "," : "") << "\n";
            continue;
        }
        os << ",\n";
        os << "          \"single_phase\": " << (r.single_phase ? "true" : "false") << ",\n";
        os << "          \"L_liquid\": " << num(r.L) << ",\n";
        os << "          \"beta_vapour\": " << num(beta) << ",\n";
        os << "          \"x_liquid\": ";
        emit_array(os, r.x);
        os << ",\n";
        os << "          \"y_vapour\": ";
        emit_array(os, r.y);
        os << ",\n";
        os << "          \"dL_liquid_du\": ";
        emit_array(os, r.dL);
        os << ",\n";
        os << "          \"dx_liquid_du\": ";
        emit_matrix(os, r.dx);
        os << ",\n";
        os << "          \"dy_vapour_du\": ";
        emit_matrix(os, r.dy);
        os << ",\n";
        for (int ph = 0; ph < 2; ++ph) {
            const char* tag = (ph == FluidSystem::oilPhaseIdx) ? "liquid" : "vapour";
            os << "          \"" << tag << "\": {\"molar_volume\": " << num(r.vm[ph])
               << ", \"molar_density\": " << num(r.rho_molar[ph])
               << ", \"mass_density\": " << num(r.rho_mass[ph])
               << ", \"z_factor\": " << num(r.z_factor[ph]) << ", \"eos_A\": " << num(r.eos_A[ph])
               << ", \"eos_B\": " << num(r.eos_B[ph]) << ", \"viscosity\": " << num(r.mu[ph])
               << ", \"fugacity_coefficient\": ";
            emit_array(os, r.phi[ph]);
            os << ", \"dmolar_density_du\": ";
            emit_array(os, r.drho_molar[ph]);
            os << ", \"dmass_density_du\": ";
            emit_array(os, r.drho_mass[ph]);
            os << ", \"dviscosity_du\": ";
            emit_array(os, r.dmu[ph]);
            os << "}" << (ph == 0 ? "," : "") << "\n";
        }
        os << "        }" << (n + 1 < cases.size() ? "," : "") << "\n";
    }
    os << "      ]\n";
    os << "    }" << (last ? "" : ",") << "\n";
}

int main(int argc, char** argv)
{
    // Defaults match what COMPOSITIONAL_VALIDATION.md pins. Overridable so a regeneration that
    // changed a knob shows up in the command line recorded next to the output, not buried in a
    // rebuild.
    Scalar tol = 1e-9;
    std::vector<std::string> methods = {"ssi+newton", "ssi"};
    for (int i = 1; i < argc; ++i) {
        const std::string a = argv[i];
        if (a == "--tolerance" && i + 1 < argc) {
            tol = std::stod(argv[++i]);
        } else if (a == "--method" && i + 1 < argc) {
            methods = {argv[++i]};
        } else {
            std::cerr << "unknown argument: " << a << "\n";
            return 2;
        }
    }

    const EOSType eos_type = EOSType::PR;
    const Scalar T = 423.15;  // 150 C, the fixed reservoir temperature pinned by C0.

    using Ternary = Opm::ThreeComponentFluidSystem<Scalar>;
    using Binary = ressim::TwoComponentFluidSystem<Scalar>;

    std::vector<Case<3>> t3;
    auto add3 = [&](const std::string& id, Scalar p_bar, Scalar t_k, Scalar z0, Scalar z1,
                    Scalar z2) { t3.push_back(Case<3>{id, p_bar * 1e5, t_k, {z0, z1, z2}}); };

    // Pressure traverse at fixed composition: crosses a phase boundary inside the traverse, so
    // the fixture set contains a real bubble/dew crossing and not only interior states.
    for (const Scalar p : {10.0, 25.0, 50.0, 75.0, 90.0, 110.0, 150.0, 200.0, 250.0, 300.0, 400.0,
                           500.0}) {
        add3("ternary_p" + std::to_string(static_cast<int>(p)), p, T, 0.2, 0.5, 0.3);
    }
    // Composition traverse at fixed pressure, light-rich to heavy-rich, CO2 held at 0.1. The
    // endpoints stop short of exactly zero: see "Oracle domain limits" in README.md.
    for (int k = 0; k <= 10; ++k) {
        const Scalar f = 0.001 + 0.998 * (k / 10.0);
        add3("ternary_zsweep" + std::to_string(k), 150.0, T, 0.1, 0.9 * f, 0.9 * (1.0 - f));
    }
    // Trace components: 1e-6 of a component is present, not absent. C1's active-component policy
    // and C3's trace handling are both checked here.
    add3("ternary_trace_co2", 150.0, T, 1e-6, 0.6, 0.399999);
    add3("ternary_trace_c10", 150.0, T, 0.1, 0.899999, 1e-6);
    // A second isotherm, to prove the Rust side's temperature reaches the EOS and is not
    // accidentally pinned to the primary fixture temperature.
    for (const Scalar p : {50.0, 150.0, 300.0}) {
        add3("ternary_T333_p" + std::to_string(static_cast<int>(p)), p, 333.15, 0.2, 0.5, 0.3);
    }

    std::vector<Case<2>> t2;
    auto add2 = [&](const std::string& id, Scalar p_bar, Scalar t_k, Scalar z0, Scalar z1) {
        t2.push_back(Case<2>{id, p_bar * 1e5, t_k, {z0, z1}});
    };
    for (const Scalar p : {20.0, 50.0, 80.0, 100.0, 150.0, 200.0, 250.0, 300.0}) {
        add2("binary_p" + std::to_string(static_cast<int>(p)), p, T, 0.6, 0.4);
    }
    for (int k = 0; k <= 8; ++k) {
        const Scalar f = 0.02 + 0.96 * (k / 8.0);
        add2("binary_zsweep" + std::to_string(k), 100.0, T, f, 1.0 - f);
    }
    for (const Scalar p : {50.0, 150.0}) {
        add2("binary_T333_p" + std::to_string(static_cast<int>(p)), p, 333.15, 0.6, 0.4);
    }

    std::ostream& os = std::cout;
    os << "{\n";
    os << "  \"schema\": \"ressim-compositional-ptflash-fixture/1\",\n";
    os << "  \"generator\": \"tools/opm_compositional/ptflash_harness.cpp\",\n";
    os << "  \"eos\": \"PR\",\n";
    os << "  \"flash_tolerance\": " << num(tol) << ",\n";
    os << "  \"flash_method_chain\": [";
    for (size_t i = 0; i < methods.size(); ++i) {
        os << (i ? ", " : "") << "\"" << methods[i] << "\"";
    }
    os << "],\n";
    os << "  \"units\": {\"pressure\": \"Pa\", \"temperature\": \"K\", \"molar_volume\": "
          "\"m3/mol\", \"molar_density\": \"mol/m3\", \"mass_density\": \"kg/m3\", "
          "\"viscosity\": \"Pa.s\", \"critical_volume\": \"m3/kmol\"},\n";
    os << "  \"beta_convention\": \"beta_vapour is the VAPOUR mole fraction; OPM's L_liquid is "
          "the LIQUID mole fraction; beta = 1 - L\",\n";
    os << "  \"derivative_note\": \"Derivatives are d/du with u = [p_Pa, z_0 .. z_(N-2)]; the "
          "dependent z_(N-1) was seeded with dz/dz_k = -1, so that invariant is enforced by the "
          "oracle rather than assumed by the consumer.\",\n";
    os << "  \"systems\": [\n";
    emit_system<Binary>(os, "ressim::TwoComponentFluidSystem", t2, eos_type, tol, methods, false);
    emit_system<Ternary>(os, "Opm::ThreeComponentFluidSystem", t3, eos_type, tol, methods, true);
    os << "  ]\n";
    os << "}\n";
    return 0;
}
