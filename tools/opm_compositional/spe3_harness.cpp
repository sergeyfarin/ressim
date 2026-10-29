// #51: independent OPM PTFlash admission for the SPE3 characterization. Reuse the C0 runner
// (including Wilson seeding and failure reporting), with a separately transcribed fluid table.
#define main c0_harness_main
#include "ptflash_harness.cpp"
#undef main

template<class S>
struct Spe3Fluid : Opm::BaseFluidSystem<S, Spe3Fluid<S>> {
    static constexpr int numPhases = 2, numComponents = 9, numMisciblePhases = 2,
                         numMiscibleComponents = 9, oilPhaseIdx = 0, gasPhaseIdx = 1, waterPhaseIdx = -1;
    static constexpr bool waterEnabled = false;
    template<class E> using ParameterCache = Opm::PTFlashParameterCache<E, Spe3Fluid<S>>;
    using CubicEOS = Opm::CubicEOS<S, Spe3Fluid<S>>;
    using ViscosityModel = Opm::ViscosityModels<S, Spe3Fluid<S>>;
    // Juell, Whitson & Hoda (2009), SPE 121252 Table 3. MW, Tc, Pc, Zc, omega.
    static constexpr double data[9][5] = {
        {44.01,304.21,73.82,.27400,.225}, {28.02,126.27,33.90,.29000,.040},
        {16.04,186.61,46.20,.28800,.013}, {30.07,305.33,48.80,.28500,.098},
        {44.10,369.85,42.50,.28100,.152}, {67.28,396.22,34.35,.27228,.234},
        {110.90,572.50,25.94,.26444,.332}, {170.90,630.22,16.92,.25140,.495},
        {282.10,862.61,8.61,.22436,.833}
    };
    static S molarMass(unsigned i) { return data[i][0]*1e-3; }
    static S criticalTemperature(unsigned i) { return data[i][1]; }
    static S criticalPressure(unsigned i) { return data[i][2]*1e5; }
    static S criticalVolume(unsigned i) {
        return data[i][3]*Opm::Constants<S>::R*criticalTemperature(i)/criticalPressure(i)*1e3;
    }
    static S acentricFactor(unsigned i) { return data[i][4]; }
    // Table 4, symmetric lower triangle (all unlisted entries are stated zeros).
    static S interactionCoefficient(unsigned i, unsigned j) {
        static constexpr double k[9][9] = {
            {}, {}, {.13,.036}, {.135,.05}, {.1277,.08}, {.1,.1002,.09281},
            {.1,.1,0,.00385,.00385}, {.1,.1,0,.0063,.0063}, {.1,.1,.1392,.006,.006}
        };
        return i >= j ? k[i][j] : k[j][i];
    }
    static std::string_view componentName(unsigned i) {
        static constexpr std::string_view names[] = {"CO2","N2","C1","C2","C3","C4-6","C7+1","C7+2","C7+3"};
        return names[i];
    }
    static bool phaseIsActive(unsigned i) { return i < 2; }
    static bool isCompressible(unsigned) { return true; }
    static bool isIdealMixture(unsigned) { return false; }
    static bool isIdealGas(unsigned) { return false; }
    static bool isLiquid(unsigned i) { return i == oilPhaseIdx; }
    static std::string_view phaseName(unsigned i) { return i == 0 ? "o" : "g"; }
    template<class FS, class E=typename FS::ValueType, class P=E>
    static E fugacityCoefficient(const FS& fs, const ParameterCache<P>& cache, unsigned ph, unsigned i) {
        return Opm::decay<E>(CubicEOS::computeFugacityCoefficient(fs, cache, ph, i));
    }
    template<class FS, class E=typename FS::ValueType, class P=E>
    static E viscosity(const FS& fs, const ParameterCache<P>& cache, unsigned ph) {
        return Opm::decay<E>(ViscosityModel::LBC(fs, cache, ph));
    }
};

int main() {
    using Fluid = Spe3Fluid<double>;
    const std::array<double,9> z = {.0121,.0194,.6599,.0869,.0591,.0967,.0472,.0153,.0034};
    std::vector<Case<9>> cases;
    for (double bar : {300.,260.,245.,240.,235.,230.,220.,200.,170.,140.,100.,50.}) {
        cases.push_back({"spe3_p"+std::to_string(int(bar)), bar*1e5, (200.+459.67)*5/9, z});
    }
    // PTFlash reports the raw cubic volumes. Translation is pressure independent and does not
    // change its equilibrium: Rust checks raw volumes by adding sum(x*s*b) back to its result.
    emit_system<Fluid>(std::cout, "SPE3-Juell-2009", cases, {}, EOSType::PR,
                       {1e-12, 1e-9}, {"ssi+newton","ssi"}, true);
}
