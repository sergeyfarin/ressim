# SPE3 compositional enablers (#51)

This is the fluid, separator and dew-point admission contract for
[#51](https://github.com/sergeyfarin/ressim/issues/51). The 9×9×4 reservoir scenario belongs to
[#52](https://github.com/sergeyfarin/ressim/issues/52); these small verification cases do not
validate that full reservoir or its published production history.

## Sourced characterization

`fluid::pinned::spe3()` uses the **nine-component Juell–Whitson–Hoda characterization**, not a
new fit and not an amalgam of participants' tables:

- Juell, A., Whitson, C.H., and Hoda, M.F. (2009), *Model-Based Integration and Optimization —
  Gas Cycling Benchmark*, SPE 121252-MS, **Tables 3–5**, pp. 8–9. The journal version is
  [SPE Journal 15(2), 646–657 (2010)](https://doi.org/10.2118/121252-PA);
  [the conference manuscript and tables are available here](https://www.researchgate.net/publication/237901380_Model-Based_Integration_and_Optimization-Gas-Cycling_Benchmark).
- Kenyon, D.E., and Behie, G.A. (1987), *Third SPE Comparative Solution Project: Gas Cycling
  of Retrograde Condensate Reservoirs*, [JPT 39(8), 981–997](https://doi.org/10.2118/12278-PA),
  **Table 3** for the separator and rate convention, **Table 17** for the original ARCO PR model.
  [Manuscript mirror](https://www.ipt.ntnu.no/~curtis/courses/PhD-PVT/PVT-HOT-Vienna-May-2016x/e-notes/GC-Papers/SPE12278-SPE3-comparative-study.pdf).

The ordering is `CO2, N2, C1, C2, C3, C4-6, C7+1, C7+2, C7+3`. Table 5's **Original**
composition is `SPE3_COMPOSITION`; its lean/rich alternatives are not used. Temperature is
200 °F, converted explicitly to kelvin. All critical properties, molecular weights,
acentric factors and BIPs are transcribed from Tables 3–4, including their stated zeros.
In particular the last component's molecular weight and acentric factor are taken directly
from the 2009 table; no substitution from another characterization is made.

The EOS is original Peng–Robinson (`PR`, the 1976 alpha polynomial), with the same cubic
constants used by OPM and MultiComponentFlash. The 2009 paper identifies this as the ARCO
model; the original paper explicitly names PR. Its volume shifts and critical compressibility
factors are the **published 2009 additions**, identified as estimates by that paper's footnote.
No estimate is performed here. Critical volume is `Zc R Tc/Pc`. The translation is
`V = ZRT/p − Σ x_i s_i (Ωb R Tc_i/Pc_i)`, on both phases, including composition derivatives.
The stored `z_factor` remains the untranslated cubic root. Equilibrium fugacity ratios are
unchanged by this pressure-independent translation. Viscosity is LBC at the translated density.

## Independent thermodynamic admission

`tools/opm_compositional/spe3_harness.cpp` transcribes the properties independently into an
OPM PTFlash system and reuses C0's cold-start runner. It writes `opm/compositional/spe3/ptflash.json`:
twelve pressure states on the 200 °F isotherm, spanning single vapour and retrograde two-phase
states. OPM's method and tolerance ladder are recorded per state, including any oracle failure.
`comp_spe3_c0_admission_matches_opm_ptflash` rejects any unresolved oracle state and checks
the component properties/BIPs, phase fraction, coexisting compositions, and raw cubic volumes.
OPM's raw volumes are compared by adding the published translation back to ResSim's result.

`tools/jutul/spe3_reference.jl` separately transcribes the tables and uses the pinned
JutulDarcy / MultiComponentFlash packages for a second flash reference. Its fixture includes
the **translated** volumes, both separator configurations, CVD and the 1D simulation below.
ResSim does not generate any reference number. Reference generator and environment hashes
are in `opm/compositional/spe3/manifest.json`.

OPM, MultiComponentFlash and ResSim have different gas constants. Volumes are converted by
the exact ratio `R_ResSim/R_oracle`, not a relaxed error band; mole fractions need no conversion.
The C0-style absolute split/composition and relative volume admission bands are `1e-8`.
Translated density derivatives are finite-difference checked in pressure and every independent
composition at both a single-vapour and a two-phase state.

## Three-stage separation

`spe3_separator_stages(false)` follows Kenyon–Behie Table 3: 815 psia / 80 °F, 65 psia / 80 °F,
14.7 psia / 60 °F. `true` selects the alternative 315 psia primary separator. For the eventual
reservoir case the primary switches when datum reservoir pressure drops below 2500 psia;
the fluid constructor explicitly selects the initial high-pressure train, and the case must
make the switch. There is no hidden reservoir-pressure policy in the thermodynamic module.

At each stage the entire incoming stream is flashed. Gas leaves, and only liquid feeds the
next stage. Surface liquid is the final stock-tank liquid. All removed gas components are
combined, then metered on the vapour EOS branch at the declared surface pressure/temperature.
The gas meter is **not another equilibrium separator**; otherwise cooling the collected gas
would change the specified process by adding a fourth condensation stage. Intermediate gas
volumes are never added across different pressures.

`FluidSpecification::with_separator_stages` requires positive finite P/T, decreasing stage
pressures, and a final stage equal to the declared surface conditions. Without stages,
`surface_separation` keeps its single-stage convention. Compositional well surface controls
already call this conversion, so a specification with a train uses it for both rates and
reporting. A train is part of the serialized specification and survives component permutation.

The independent Julia flash runs both primary configurations. Tests compare each collected
component and both surface volumes, close every component balance, and cover zero stream,
scaling, and permutation. These are same-EOS verification references, not a claim that the
2009 characterization exactly reproduces the 1987 separator laboratory yields.

## CVD and 1D dew-point crossing

The constant-volume experiment starts with one mole of the original composition at 260 bar.
Each lower pressure is flashed, equilibrium vapour is withdrawn to restore the original volume,
and the retained mixture is flashed again. The fixture records composition, retained moles,
withdrawal and liquid volume fraction. Tests compare these against the independent flash,
close every component balance, and verify that re-equilibrated retained material fills the
original volume. This is a model-to-model CVD check, not a fit to the SPE3 laboratory curve.

The dynamic reference is a three-cell 1D column: 10 m cubic cells, 10 mD, porosity 0.2, fixed
pore volume, no gravity/capillary pressure, initial pressure 260 bar, and the sourced fluid.
A gas-only producer on cell 3 has BHP 140 bar and geometric well index
0.001 m³·cP/(day·bar). Liquid is immobile (`krL = 0`, `krV = Sv`); this is an explicit
verification constitutive law, not the SPE3 reservoir's relative permeability. Both engines
use the same grid, well index, backward-Euler schedule (150 one-day reports) and LBC.
JutulDarcy uses CNV `1e-9` / MB `1e-10`; ResSim's component-scaled Newton target is `1e-10`.

`comp_spe3_depletion_crosses_dew_point_without_fragmentation_against_jutul` compares **each
cell at every report** in pressure, liquid saturation and all nine overall fractions. Its
acceptance bands are 0.01 bar, `1e-4` saturation and `1e-5` composition. It requires single-phase
and two-phase states, all 150 report intervals accepted without retry or timestep cut, and
independent final-inventory-plus-production closure for every component at `1e-8` relative.
The test prints the measured worst gaps and Newton count; the committed oracle retains all
states. The existing `flowexp_comp` saturation-crossing limitation is not used as an oracle
here: the acceptance reference is JutulDarcy compositional.

## Gas cycling decision

Use **fixed separator-gas injection rate**, Case 1's 4700 Mscf/day during its ten-year cycling
period, then shut the injector for blowdown. This follows the original study and OPM deck.
Do not introduce a controller equating injection to the instantaneous produced gas minus sales:
that changes the benchmark when the producer reaches its BHP constraint. A future production
feedback/reinjection study must be a separately named sensitivity. #52 owns the reservoir
schedule and injection-composition wiring; this enabler does not claim that full run.

## Replay and gates

```bash
# Regenerate both independent references (needs OPM headers and Julia 1.12):
bash tools/opm_compositional/run-spe3.sh
# Verify both; reruns JutulDarcy, including the full depletion schedule:
bash tools/opm_compositional/run-spe3.sh --check
# OPM-only regeneration check, included in the ordinary compositional fixture gate:
bash tools/opm_compositional/run-spe3.sh --check --opm-only
# Acceptance contracts and measured gaps:
cargo test --release --manifest-path src/lib/ressim/Cargo.toml comp_spe3_ -- --nocapture
bash scripts/validate-compositional.sh native
```

The normal `thermo` gate includes every `comp_spe3_*` test. Julia is an offline fixture producer,
not a browser dependency. The public scenario picker remains governed by #52's admission gates.
