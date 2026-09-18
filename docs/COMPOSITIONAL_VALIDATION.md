# Compositional validation

Owning document for the compositional model's fluid dataset, numerical acceptance contract,
oracles and gate results. Created by task **C0** of the
[compositional fluid execution plan](COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md), whose
[2026-09-16 corrections](COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md#plan-corrections--2026-09-16)
pin the dataset and oracle recorded here. Tracking issue:
[#29](https://github.com/sergeyfarin/ressim/issues/29).

**Every claim here names what it does and does not cover.** The status table in section 7 records
the state of each task and the completion records in section 8 record how each one was measured.
C0–C11 are complete; C12 is partial — its thermodynamic comparison, its transport comparison and
both refinement studies against an independent simulator are done, but the plan's cumulative
acceptance target is **INCONCLUSIVE** on this reference and the `NATIVE-COMPOSITIONAL-READY`
milestone is **not** declared. C13 onwards have not started.

## Scope this document governs

Isothermal hydrocarbon compositional flow (**V1**): Peng–Robinson EOS, 2 or 3 components, one or
two hydrocarbon phases, phase appearance and disappearance, composition-dependent density and
viscosity, component-conservative fully implicit transport. Cartesian geometry, one fluid region,
constant prescribed temperature, no water, no hydrocarbon capillary pressure, zero gravity until
C9's own subtask enables it. Immiscible water is the separate V1b extension (C15).

CO2 appears in the fluid below as a PR-EOS component of a hydrocarbon system at fixed temperature.
This is **not** aqueous CO2. Brine, CO2 dissolution and any storage application remain out of
scope and would need a different thermodynamic model, not a parameter change.

## 1. The pinned fluid

Two fluid systems, both sourced entirely from the installed OPM 2026.04 packages. No property
below was chosen, fitted or rounded by ResSim.

### Ternary (N=3) — `Opm::ThreeComponentFluidSystem`

`/usr/include/opm/material/fluidsystems/ThreeComponentFluidSystem.hh`, owned by
`libopm-common-dev 2026.04-1~noble`, © SINTEF Digital 2022, GPL-2+. Used unmodified.

| Index | Component | Source header | MW [kg/mol] | Tc [K] | pc [Pa] | Vc [m³/kmol] | ω |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | CO2 | `components/SimpleCO2.hpp` | 0.044 | 304.10 | 7.38e6 | 9.412e-2 | 0.224 |
| 1 | C1 (methane) | `components/C1.hpp` | 0.0160 | 190.6 | 4.60e6 | 9.863e-2 | 0.011 |
| 2 | C10 (n-decane) | `components/C10.hpp` | 0.142 | 617.7 | 2.10e6 | 6.098e-1 | 0.488 |

`SimpleCO2::criticalTemperature()` is written as `273.15 + 30.95`.

### Binary (N=2) — `ressim::TwoComponentFluidSystem`

C1 and C10 only, components 1 and 2 above with the same values. Declared in
`tools/opm_compositional/two_component_fluid_system.hh` because OPM ships no two-component
system and its stability test cannot handle `z = 0` (see §4). It delegates every property to
OPM's own component classes and introduces no data.

### Binary interaction matrix

All zeros, for both systems. `ThreeComponentFluidSystem::interactionCoefficient()` returns `0.0`
unconditionally. This is that source's explicit modelling choice, which is what makes it citable.
Zero BIPs are never a default, and a future fluid with nonzero BIPs needs its own source.

### EOS variant and constants

`CompositionalConfig::EOSType::PR` — unmodified Peng–Robinson. From
`/usr/include/opm/material/eos/PRParams.hpp`:

```text
Omega_A(T, i) = 0.457235529 * [1 + f(w_i) * (1 - sqrt(T/Tc_i))]^2
Omega_B        = 0.077796074
f(w)           = 0.37464 + 1.54226 w - 0.26992 w^2
m1, m2         = 1 + sqrt(2), 1 - sqrt(2)
```

`PRCORR` swaps `f(w)` for a quartic when `w > 0.49`. C10's `w = 0.488` sits just under that, so
PR and PRCORR coincide on this fluid. That coincidence is not a reason to leave the variant
unpinned — V1 is PR.

Fugacity coefficient, from `eos/CubicEOS.hpp`:

```text
ln phi_i = -ln(Z - B) + (Bi/B)(Z - 1)
           + [ln((Z + m2 B)/(Z + m1 B)) * A / ((m1 - m2) B)] * ((2/A) sum_j A_ij y_j - Bi/B)
```

Root labelling, from `CubicEOS::computeMolarVolume`: with three real roots, the largest `Z` is the
gas branch and the smallest the liquid branch; with one real root, that root serves both. This is
*labelling*, not a stability decision — C2's requirement that two algebraic roots are not two
coexisting phases stands.

### Viscosity

Lohrenz–Bray–Clark, `/usr/include/opm/material/viscositymodels/LBC.hpp`, citing Lohrenz, Bray &
Clark, JPT 16.10 (1964), with OPM's noted correction of the paper's `-0.40758` typo to `-0.040758`.
Coefficients `{0.10230, 0.023364, 0.058533, -0.040758, 0.0093324}`. It consumes `criticalVolume()`
as **m³/kmol** (it divides by 1000 to reach m³/mol), which is why the tables above carry that unit.
LBC also reads the compressibility factor off the fluid state; it is not derived internally.

### Conditions

| | |
| --- | --- |
| Reservoir temperature | 423.15 K (150 °C), fixed |
| Second fixture isotherm | 333.15 K (60 °C) — proves temperature reaches the EOS |
| Pressure envelope | 10–500 bar (1e6–5e7 Pa) |
| Surface conditions | **not yet pinned** — owed by C5, see §6 |

## 2. Symbol and unit conventions

The two that have already caused confusion in the source material:

- **`beta` is the vapour mole fraction.** OPM's `L` is the **liquid** mole fraction and its
  `rachfordRice_g_` solves for `L`. `beta = 1 - L`. The fixture writes both fields out so nothing
  downstream infers which is which.
- **The gas constant differs from the oracle's.** OPM's `Constants.hpp` uses the superseded
  `R = 8.314472`; ResSim uses the exact SI `8.31446261815324`. The difference is **+1.128e-6
  relative**, an order of magnitude above the 1e-7 density target. It is handled by an exact
  conversion rather than a widened tolerance, because the dependence is analytic: `A_i = Omega_A
  p_r / T_r^2` and `B_i = Omega_B p_r / T_r` contain no `R`, so `Z`, `A`, `B` and every fugacity
  coefficient are `R`-independent, and only `V_m = Z R T / p` and its reciprocal carry the factor,
  linearly. A C2 test asserts the uncorrected density discrepancy is that ratio and nothing else,
  which turns the mismatch into a check on where `R` enters. LBC viscosity depends on `R`
  non-linearly through the reduced density, so C5 must handle it separately.
- **Flash phase fraction is not saturation.** `S_vapour = (beta/cV) / v_mixture` with
  `v_mixture = (1-beta)/cL + beta/cV`. Never set `S_vapour = beta`.

Unit boundaries are as specified in the plan: SI inside the thermodynamics kernel (Pa, K, mol, m³,
kg/mol, J/(mol·K)); m, mD, bar, cP, day outside it; `p_Pa = 1e5 * p_bar`;
`df/dp_bar = 1e5 * df/dp_Pa`; `mu_cP = 1e3 * mu_Pa_s`. The fixture is SI throughout and states its
units in its own `units` field.

## 3. Oracles

### 3a. Thermodynamic oracle — AVAILABLE

OPM's PTFlash / CubicEOS / LBC stack, called directly through a header-only harness. No OPM build,
no link against `flow`, no MPI.

```bash
bash scripts/validate-compositional.sh all        # the whole compositional gate
bash scripts/validate-compositional.sh thermo     # tests only, ~20 s
bash tools/opm_compositional/generate.sh          # regenerate the fixture
bash tools/opm_compositional/generate.sh --check  # verify the committed fixture reproduces
```

| | |
| --- | --- |
| Harness | `tools/opm_compositional/ptflash_harness.cpp`, `two_component_fluid_system.hh` |
| Fixture | `opm/compositional/ptflash_fixtures.json` |
| Provenance | `opm/compositional/manifest.json` — compiler, package versions, sha256, source commit |
| Packages | `libopm-common-dev` / `libopm-simulators-dev` 2026.04-1~noble, `libdune-common-dev` 2.11.0-1~noble |
| Compiler | `g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0`, `-std=c++20` (C++17 does not compile) |

**Flashed states: 47** — 29 two-phase, 15 single-liquid, 3 single-vapour — across both fluid
systems, two isotherms, a 10–500 bar pressure traverse that crosses a phase boundary, composition
sweeps from light-rich to heavy-rich, and trace-component states at 1e-6. Each carries
`L`/`beta`, `x`, `y`, per-phase molar volume, molar and mass density, Z factor, EOS `A`/`B`,
fugacity coefficients, LBC viscosity, and analytic derivatives of `L`, `x`, `y`, molar density,
mass density and viscosity with respect to `u = [p_Pa, z_0 .. z_(N-2)]`.

**Flash-free EOS states: 21**, added during C2 (schema `/2`). These evaluate OPM's cubic directly
at a given `(p, T, x)` with no stability test, and report both extreme roots. They exist because
**none of the 47 flashed states has three real roots** — at 150 °C every one is monotonic, so a
port could get the root-labelling rule completely wrong and still reproduce the entire flashed
fixture. C2 requires a multi-root state explicitly, and seven of these are genuinely multi-root:
pure n-decane below its vapour pressure at `T_r = 0.685`, plus C10-rich binary and ternary
mixtures in the same region. Pure methane is supercritical there and gives one root at every
pressure, which is the contrast that makes the test meaningful. They also cover pure components,
which PTFlash cannot flash at all (§4, limit 1) but the EOS evaluates without difficulty.

**Derivatives are a first-class part of this oracle, not a bonus.** PTFlash reconstructs them by
implicit differentiation of the converged equilibrium, which is the pattern C4 is instructed to
follow. The harness seeds the dependent `z_(N-1)` with `dz_(N-1)/dz_k = -1`, so C4's required
invariant is enforced by the oracle rather than assumed by its consumer.

### 3b. Trajectory oracle — AVAILABLE since 2026-09-16

**This was BLOCKED and is no longer.** Nothing is packaged — `libopm-simulators-bin` ships only
the black-oil `/usr/bin/flow` — so the compositional simulator was **built from pinned upstream
source**, on the maintainer's instruction.

```bash
bash tools/opm_compositional/build-flowexp-comp.sh
```

| | |
| --- | --- |
| Executable | `flowexp_comp` — **not** `flow_comp`, which is what the plan guessed |
| Source | `OPM/opm-simulators`, tag `release/2026.04/final`, commit `b82f21dba405286c4c4446614dd3bf9cdebf7a2c` |
| Why that tag | it matches the installed `libopm-common-dev` / `libopm-simulators-dev` 2026.04 packages the headers come from |
| Configure | `-DCMAKE_BUILD_TYPE=Release -DOPM_COMPILE_COMPONENTS="2;3" -DOPM_ENABLE_PYTHON=OFF -DBUILD_TESTING=OFF` |
| Build-tree patches | three, all recorded in the script, none changing OPM behaviour: two unconditional CMake hooks that assume a full build, and two test targets that link `Boost::unit_test_framework`, which is not installed |
| Location | outside the repository (`../ressim-opm-build/`), since it is ~1 GB of build tree |

**It runs.** `OPM/opm-tests`'s `compositional/1D_COMP.DATA` — a five-cell 1D CO2 flood in CO2 /
methane / decane, which is ResSim's own V1 fluid — completes 28 report steps in 0.2 s and writes
`UNRST`, `SMSPEC`/`UNSMRY` and `ESMRY`.

**Three mismatches between that deck and ResSim's pinned fluid, which C12 must respect rather than
paper over:**

1. **The EOS data is not identical.** The deck carries more precise values than
   `ThreeComponentFluidSystem`'s hard-coded ones — e.g. `ACF = [0.22394, 0.01142, 0.4884]` against
   `[0.224, 0.011, 0.488]`, and `PCRIT = [73.773, 45.992, 21.03]` bar against
   `[73.8, 46.0, 21.0]`. Critical volumes and the zero interaction matrix do match. C12 must build
   its fluid from **the deck's** numbers, because the deck is what the oracle ran.
2. **The deck supplies its own relative permeability.** `SGOF` is a Corey-squared table keyed on
   gas saturation. This is precisely the "benchmark cases can supply their own curves" the
   pluggable `RelativePermeabilityModel` exists for; the table is keyed on liquid saturation here,
   so `S_L = 1 - S_g`.
3. **The deck supplies its own surface conditions.** `STCOND 15.0 1.0` is 15 °C and 1 bar, not the
   288.71 K / 1 atm pinned in §6. Another quantity that is data rather than a constant.

### 3c. Independent invariants

Available now and not dependent on either oracle: composition normalization, `z` reconstruction
from `L`, `x`, `y`; fugacity equality between coexisting phases; ideal-gas and pure-component
limits; dimensional checks; analytic-vs-FD derivative agreement; local and global mole
conservation; component-permutation invariance.

Single-component states have **no** external oracle here (§4), but a pure PR root is its own
analytic oracle: `A` and `B` reduce to closed forms in reduced temperature and pressure.

## 4. Measured oracle domain limits

Measured on the committed fixture, not assumed. The Rust implementation is not required to share
these limits — but a state OPM cannot resolve has no external referee and needs an invariant
instead.

| Limit | Evidence | Consequence |
| --- | --- | --- |
| An exactly zero component is unsupported **by the flash** | `checkStability_` forms `z_i/K_i` and `K_i*z_i`; every `z_i = 0` state throws `Stability test did not converge`. OPM's own TODO: "make sure that no mole fraction is smaller than 1e-8?" | The binary is its own fluid system, not the ternary with `z_CO2 = 0`. Composition sweeps stop at 0.001. **C1's active-component policy for true zeros and C3's zero-component handling cannot be validated against the flash.** The *EOS* has no such limit, and the flash-free section covers pure components — so this constrains C3, not C2 |
| `K` must be seeded by the caller | PTFlash never applies its own `wilsonK_`; `K = 0` fails every state | "Cold start" in this fixture means the Wilson correlation, not zero |
| Flash tolerance floor ≈ 1e-9 | At `--tolerance 1e-11`, 27 of 47 states fail `Newton composition update did not converge`; at `1e-9` all 47 resolve | Pinned tolerance is `1e-9`. This caps the achievable equilibrium residual — see §5 |
| Six states need the SSI-only path | Method chain is `ssi+newton` then `ssi`; `method_used` is recorded per state | No fixture value is anonymous about how it was reached |
| `Z` must be written before viscosity | LBC reads `compressFactor`; nothing in PTFlash sets it | Without the write, every viscosity is `NaN` |
| OPM clamps `phi` to `[1e-10, 1e10]` | `CubicEOS.hpp` | No fixture state reaches the clamp. **The Rust port must not copy it** (plan rule 6). A future clamped state would be excluded, not matched |

## 5. Numerical acceptance contract

The plan proposed these targets and required C0 to confirm the fixture can actually resolve them
before implementation. Column 3 is what the committed fixture measures **of itself** — the
oracle's own quality, which bounds any target a test can hold ResSim to.

| Quantity | Admission target | Oracle's own measured value | Confirmed? |
| --- | --- | --- | --- |
| Composition normalization | ≤ 1e-12 absolute | 2.2e-16 (`ternary_p90`) | Yes, 4 orders of margin |
| Flash `z` reconstruction | ≤ 1e-10 max absolute per component | 1.1e-16 (`ternary_p90`) | Yes, 6 orders of margin |
| Two-phase equilibrium | ≤ 1e-8 max abs log-fugacity ratio | 5.1e-10 (`binary_p100`) | Yes, ~20x margin. Set by the 1e-9 flash tolerance floor; a tighter target would need a better reference |
| Scalar EOS/flash fixture | 1e-7 relative on density/fugacity, 1e-8 absolute on `beta`/composition | Fixture stores 17 significant digits, so serialization never limits the test; the reference's own equilibrium quality is 5.1e-10 | Yes |
| **C2 EOS vs fixture** — mixture `A`, `B` | — | `< 1e-13` relative over 188 comparisons | **Met** |
| **C2 EOS vs fixture** — compressibility factor | — | `< 1e-12` relative over 94 branch evaluations | **Met** |
| **C2 EOS vs fixture** — fugacity coefficients | 1e-7 relative | `< 1e-11` relative over 244 comparisons | **Met**, 4 orders of margin |
| **C2 EOS vs fixture** — densities | 1e-7 relative | `< 1e-7` after the exact gas-constant correction; the uncorrected discrepancy is a pure ratio to `< 1e-12` | **Met** |
| **C2 EOS vs fixture** — flash-free states | — | `< 1e-11` relative over 150+ comparisons including 7 multi-root states | **Met** |
| **C3 flash vs fixture** — phase state | — | 47/47 states, 29 two-phase / 15 liquid / 3 vapour | **Met** |
| **C3 flash vs fixture** — vapour fraction | 1e-8 absolute | `< 1e-8` absolute on `L`, converted from `beta` | **Met** |
| **C3 flash vs fixture** — phase compositions | 1e-8 absolute | `< 1e-8` absolute on every `x_i` and `y_i` | **Met** |
| **C3 flash** — own equilibrium residual | 1e-8 on the log-fugacity ratio | `< 1e-11`, three orders inside the target | **Met** |
| **C3 flash** — normalization, `z` reconstruction | 1e-12, 1e-10 | `<= 1e-12`, `<= 1e-10` | **Met** |
| **C4 derivatives vs fixture** — `dbeta`, `dx`, `dy` | 1e-4 relative on a frozen scale | `< 1e-9`; worst 8.7e-10 at `binary_p100/dbeta/dp` | **Met**, five orders of margin |
| **C4 derivatives vs fixture** — molar density | 1e-4 relative | `< 1e-9`; worst 3.4e-10, after the same exact `R` correction | **Met** |
| **C4 derivatives** — FD plateau | plateau demonstrated, not a single step | `dbeta/dp` agrees to `< 1e-7` with at least 4 of 8 steps inside 1e-5 | **Met** |
| **C4 derivatives** — `sum_i dx_i/du = 0` | — | `< 1e-12` relative to the column's own magnitude | **Met** |
| **C4 derivatives** — `dz_(N-1)/dz_k = -1` | exact | exact, on all 18 single-phase states | **Met** |
| **C4 conditioning** — equilibrium Jacobian | — | min pivot `> 1e-6` across all 29 two-phase states | Recorded |
| **C4 conditioning** — cubic root separation | — | min `\|dP/dZ\|` `> 1e-3` across all 47 states | Recorded |
| **C5 LBC vs fixture** | 1e-7 relative | `< 1e-13` relative over 94 comparisons, evaluated at OPM's own molar density | **Met** |
| **C5 LBC** — gas-constant sensitivity | — | the `R` shift moves viscosity by `< 1e-5` relative; **not** a clean ratio, since LBC's density dependence is a quartic | Recorded |
| **C5 saturations** | reconstruction exact | `< 1e-14` against an independent volume split; `S_V - beta` reaches `> 0.3` | **Met** |
| **C5 surface** — component conservation | — | `<= 1e-9` relative per component, on binary and ternary streams | **Met** |
| **C5 surface** — gas molar volume | — | within 2% of `RT/p` at 1 atm, as an ideal-gas sanity anchor | **Met** |
| **C6 sweep** — binary, 2 940 samples | every sample classified | 0 failures: 743 two-phase, 1 901 liquid, 296 vapour | **Met** |
| **C6 sweep** — ternary, 4 680 samples | every sample classified | 0 failures: 1 273 two-phase, 2 826 liquid, 581 vapour | **Met** |
| **C6 sweep** — `z` reconstruction | 1e-10 | 1.1e-14 worst over 7 620 samples | **Met** |
| **C6 sweep** — `sum_i dx_i/du = 0` | — | 2.9e-16 worst | **Met** |
| **C6 sweep** — equilibrium-Jacobian pivot | — | 4.8e-4 worst | Recorded |
| **C6 sweep** — substitution iterations | bounded below the 5 000 cap | 747 worst | Recorded |
| **C6 trace components** | conserved | `< 1e-8` relative down to `z_i = 1e-10` | **Met** |
| **C7 layout** — indexing | bijective for N=2,3,4 | every cell column and row reached exactly once, round-tripping through the split | **Met** |
| **C7 layout** — shared metadata | identical partition | `FimLinearBlockLayout` and `CompositionalLayout` agree on every cell column, for all three N | **Met** |
| **C7 state** — cache neutrality | results unchanged | bit-identical flash results with the cache dropped | **Met** |
| **C8 inventory** — phase-sum vs `PV z_i / v_mix` | roundoff | `< 1e-12` relative across all three phase regimes | **Met** |
| **C8 residual** — stationary closed cell | exactly zero | exactly zero in all three regimes | **Met** |
| **C8 Jacobian** — vs FD | 1e-5 scaled entrywise | `< 1e-5` over every entry, both phase regimes, pressure and composition columns | **Met** |
| **C8 scaling** — trace component visibility | a trace component must not hide | a 20% error in a component holding 1e-4 of the cell reports `> 0.1`, where a cell-wide scale reports `< 1e-5` | **Met** |
| **C9 flux** — local conservative exchange | opposite contributions cancel | exactly, from one flux inserted with two signs | **Met** |
| **C9 flux** — Jacobian vs FD | 1e-5 scaled | `< 1e-4` over every entry of the `N x 2N` face block, both neighbours, both regimes | **Met** |
| **C9 assembly** — closed-grid closure | — | internal faces cancel exactly; the residual sums to the source term to `< 1e-9` relative | **Met** |
| **C9 assembly** — Jacobian vs numerical | 1e-5 scaled entrywise | `< 1e-5` on 2- and 3-cell grids, binary and ternary, column-scaled | **Met** |
| **C9 assembly** — sparsity | neighbours only | every non-adjacent entry is exactly zero | **Met** |
| **Relperm** — pluggability | caller-supplied, no engine default | `Linear`/`Corey`/`Tabulated`, no `Default` impl, all parameters required and validated; flux gives different answers under different models | **Met** |
| **C12 vs OPM** — phase state | must agree | 140 states across a full CO2 displacement; every two-phase/single-phase classification agrees | **Met** |
| **C12 vs OPM** — vapour saturation | — | worst **1.31e-7** absolute; the reference writes single precision, so this is its own output resolution | **Met** |
| **C12 vs OPM** — phase compositions | 1e-8 absolute (C0 target) | worst **5.55e-8** absolute, likewise at the reference's float32 floor | **Met** at the oracle's precision |
| **C12 vs OPM** — initial state | — | `S_g`, `x` and `y` agree to every one of the 8 digits OPM printed | **Met** |
| **C12 vs OPM** — final pressures | two settled solutions must agree | worst **0.003 bar** on the deck's grid; **≤ 0.031 bar** at every resolution from 5 to 40 cells, with no drift | **Met** |
| **C12 vs OPM** — developed displacement | — | worst **3.708 bar** from 2 days on (cell 3, 14.11 d) | **Met** |
| **C12 vs OPM** — startup transient | — | worst **3.970 bar** (cell 0, 0.35 d). It was 20 bar until the refinement study exposed two driver defects — a well-opening tolerance too tight for a single-precision report time, and `COMPDAT` item 9 read as a radius when it is a diameter | **Met**, and reported separately rather than folded into one number |
| **C12 vs OPM** — CO2 front | — | worst **0.1327** in `z_CO2` (cell 4, 17.11 d); on five cells a small difference in front arrival reads as a large composition difference in the cell the front is crossing | **Met** |
| **C12 vs OPM** — CO2 held by the grid | — | **≤ 0.02%** at every resolution from 5 to 40 cells. Conversion-free, so this is the load-bearing transport result | **Met** |
| **C12 vs OPM** — cumulative injection | ≤ 1% on cumulative quantities | **1.10%** on `FGIT` on the deck's grid, rising to **5.12%** at 40 cells. `FGIT` is a surface volume and the conversion is not shared; `GAS_DEN` comes back identically zero so the reference's own surface density cannot be read | **INCONCLUSIVE** per the plan's own rule for state/units mismatches — neither met nor failed, but *unevaluated*. See the C12 record |
| **C10 Newton** — convergence rate | quadratic near the solution | a quadratic reduction is observed and asserted; a merely-close Jacobian would converge linearly and still terminate | **Met** |
| **C10 Newton** — domain | `p > 0`, `z` on the simplex including `z_(N-1)` | fraction-to-boundary at 0.99, one global scale factor, no clamping or renormalization after the step | **Met** |
| **C10 lifecycle** — rejected step | nothing mutates | state, clock and cumulative totals all unchanged, structurally | **Met** |
| **C10 lifecycle** — multi-step closure | — | over 5 accepted steps, what the grid lost equals what was produced to `< 1e-6` relative | **Met** |
| **C10 diagnostics** — failure classification | five kinds distinguished | flash, linear, nonlinear, admissibility and budget are separate, and budget carries the underlying failure | **Met** |
| **C9 gravity** — single-phase hydrostatic | stationary at analytic equilibrium | liquid potential `< 1e-10` bar, flux `< 1e-8` mol/day; a 4-cell column assembles to a scaled residual `< 1e-12` | **Met** |
| **C9 gravity** — head uses mass density | analytic | matches `rho_mass g dz 1e-5` to `< 1e-9` relative; the molar density is 10x larger and would not | **Met** |
| **C9 gravity** — Jacobian vs FD | 1e-5 scaled | `< 1e-4` including the new downstream-composition coupling | **Met** |
| **C11 wells** — derivatives | AD/FD agreement | `< 1e-4` on the `N x N` cell block and the `N x 1` BHP column, producer and injector | **Met** |
| **C11 wells** — injected composition | exactly the prescribed one | `< 1e-12` on every component, for three different cell compositions. The mobility is one scalar multiplying every component, so taking it from the cell changes how much enters and never what enters | **Met** |
| **C11 wells** — injectivity | must not vanish | a vapour injector into a single-phase liquid cell still injects — the failure the plan names. Since C12 the mobility is the perforation cell's **total** mobility (OPM's law), which is a sum over the phases present and so cannot vanish for the reason the plan warns about | **Met** |
| **C11 wells** — inventory closure | — | over 5 accepted steps, the grid's loss equals the well's production to `< 1e-6` relative | **Met** |
| **C11 wells** — control switching | limit overrides target | a binding BHP limit reverts to BHP control and reports the achieved rate, not the target | **Met** |
| **C11 wells** — crossflow | implemented **or** explicitly rejected, never clipped | **rejected**, with a typed error naming the completion, cell and potential. A single-completion well at or above cell pressure is shut in instead, since nothing is being invented there | **Met** |
| **C11 wells** — multi-completion | one shared BHP | rates scale with each completion's index; head offsets differentiate the connections; rate control targets the well total | **Met** |
| Derivative closure | — | `sum_i d(x_i)/du_v` and `sum_i d(y_i)/du_v` worst 6.1e-16 (`binary_T333_p150`) | The oracle's derivatives satisfy the normalization identity to roundoff |
| Smooth property derivatives | ≤ 1e-4 relative, on a frozen nonzero derivative scale, over an FD step plateau | Deferred to C4 — needs the Rust implementation to compare against | Not yet |
| Tiny assembled Jacobian | ≤ 1e-5 scaled entrywise vs FD at smooth states | Deferred to C8/C9 | Not yet |
| Tiny direct linear oracle | ≤ 1e-10 full-system relative residual | Deferred to C10 | Not yet |
| Local conservative face exchange | Cancellation to roundoff | Deferred to C9 | Not yet |
| Global closed/source balance | ≤ 1e-8 relative accumulated mole error per component | Deferred to C9/C10 | Not yet |
| Refined external trajectories | ≤ 1% on cumulative quantities, per-observable tolerances in C12 | **Done.** Sub-step halved four times (first-order convergence confirmed; the comparison step is on the converged part of the curve) and the grid refined 5 → 10 → 20 → 40 with the reference re-solved on each. Per-observable bands frozen from those measurements (rows above). The cumulative target itself is unevaluable on this reference | Partial — the study is complete, the cumulative target is INCONCLUSIVE |
| Newton acceptance | Derived and frozen in C8/C11 | Deferred | Not yet |

No existing black-oil benchmark tolerance is changed by any of this.

## 6. Open decisions this document owes

1. ~~**Surface conditions are not pinned.**~~ **CLOSED by C5.** `1 atm = 101 325 Pa` and
   `288.71 K` (15.56 °C), a **single equilibrium stage**. Sourced, not chosen:
   `opm/input/eclipse/EclipseState/Compositional/CompositionalConfig.hpp` sets
   `standard_pressure = 1 * unit::atm` and `standard_temperature = 288.71` as the defaults for
   **compositional** runs specifically, and `Units.hpp` defines `atm = 101325 Pa`. These are not
   inherited from the black-oil path, whose standard-volume outputs keep their own separate
   meaning. The surface flash produces no `Bo`, `Bg` or `Rs`, and its gas/liquid ratio is
   deliberately not called GOR.
2. ~~**The hydrocarbon relative permeability law is pinned only provisionally.**~~
   **DECIDED 2026-09-16 by the maintainer.** V1 runs on **straight-line** relative permeability
   (`kr_L = S_L`, `kr_V = S_V`, no residual saturations, no endpoint scaling) as an **explicitly
   documented verification assumption**. No arbitrary Corey parameters and no residual saturations
   are introduced, and the straight-line model is **not** the physical or default model for real
   reservoirs.

   *Why straight lines for verification.* The plan forbids reusing the existing water/oil curves,
   which were fitted for a different pair of phases, and no sourced hydrocarbon liquid/vapour table
   exists in this environment. Straight lines add no fitted parameters, so an error in a
   displacement front is attributable to the thermodynamics and the discretization rather than to a
   curve nobody can cite.

   *How the decision is enforced structurally.* `compositional::relperm::RelativePermeabilityModel`
   has three variants — `Linear`, `Corey` and `Tabulated` — and **no `Default`**, because a default
   would make the most consequential unsourced assumption the one nobody had to type. `Corey` and
   `Tabulated` take every parameter as a required, validated input, so a benchmark case can supply
   its own curves and no number in that file describes any particular rock.
   `is_verification_only()` returns true for `Linear` so a scenario-admission check can refuse to
   ship a case still running on it, instead of relying on someone noticing.

   *What it is still not.* A real displacement front will be less sharp under straight lines than
   under a Corey curve. A sourced table remains preferable for any case that claims to represent a
   reservoir.
3. **The first flow fixture is specified but not built.** 1D uniform column, no gravity or
   capillarity, fixed temperature, one injection and one production boundary, composition chosen
   to force a phase change inside the declared domain. It cannot be given an external reference
   (§3b), so its acceptance rests on invariants.
4. **The iterative linear adapter is deferred.** C10 delivers the direct dense solve the plan
   asks it to start with. A component-aware block-ILU or CPR path is not implemented, and it is
   not needed until C14's performance work gives it a budget to meet: its entire value is being
   compared against a correction known to be right, which is what the direct solve now provides.
   When it lands, the plan's warning applies — do not copy black-oil quasi-IMPES weights because
   the matrix sizes happen to match.
5. ~~**The near-critical domain is unclassified.**~~ **CLOSED by C6, and declared supported** for
   the pinned fluids over 10–500 bar at 423.15 K. The C1/C10 binary's two-phase envelope closes
   between 200 and 250 bar at `z = [0.6, 0.4]`; a 0.1 bar traverse across it finds the phase
   boundary sharp (the two states do not interleave over more than 0.15 bar), every sample
   classified, and **no** degenerate derivative state. The declaration is conditional in the test
   itself: if a merged cubic root or a singular equilibrium Jacobian ever appears on that traverse,
   the test fails and the domain must be narrowed rather than the assertion relaxed.

## 7. Gate status

| Task | Deliverable | Status | Evidence |
| --- | --- | --- | --- |
| C0 | Frozen scope, fluid dataset, oracles, acceptance contract | **COMPLETE** | This document; `opm/compositional/`; `tools/opm_compositional/` |
| C1 | Fluid specification and unit-safe input | **COMPLETE** | `src/lib/ressim/src/fluid/{specification,units,pinned}.rs`; 26 `comp_spec_*` / `comp_units_*` tests |
| C2 | PR mixture EOS and single-phase properties | **COMPLETE** | `src/lib/ressim/src/fluid/eos.rs`; 22 `comp_eos_*` tests |
| C3 | Stability and scalar PT flash | **COMPLETE** | `fluid/{stability,flash}.rs`; 7 `comp_stability_*`, 12 `comp_flash_*`, 4 `comp_rr_*`; 47/47 states match OPM |
| C4 | Equilibrium and property derivatives | **COMPLETE** | `fluid/derivatives.rs`; 11 `comp_derivatives_*` tests |
| C5 | Transport properties and surface flash | **COMPLETE** | `fluid/transport.rs`; 17 `comp_transport_*` / `comp_surface_*` tests |
| C6 | **THERMO-READY** | **DECLARED** | `scripts/validate-compositional.sh`; 110 tests; 7 620-sample domain sweep. External flash parity met (§3a); **trajectory parity remains blocked** (§3b) |
| C7 | Component layout and state | **COMPLETE** | `compositional/{layout,state}.rs`; 30 `comp_layout_*` / `comp_state_*` tests |
| C8 | Cell inventory, accumulation and scaling | **COMPLETE** | `compositional/accumulation.rs`; 19 `comp_accumulation_*` / `comp_scaling_*` tests |
| C9 | Component face flux, gravity and global assembly | **COMPLETE** | `compositional/{flux,assembly}.rs`; 33 `comp_flux_*` / `comp_assembly_*` / `comp_gravity_*` tests |
| C10 | Linear solve, Newton, timestep lifecycle | **COMPLETE** on the direct path | `compositional/{newton,timestep}.rs`; 20 `comp_newton_*` / `comp_rollback_*` tests. The iterative/CPR adapter is deferred — see the C10 record |
| C11 | Compositional wells | **COMPLETE** | `compositional/wells.rs`, [design note](COMPOSITIONAL_WELL_DESIGN.md); 29 `comp_well_*` tests, single and multiple completions. **Derived from first principles — C11's OPM reference is unavailable here** (plan correction 3), so no OPM agreement is claimed |
| C12 | NATIVE-COMPOSITIONAL-READY | **PARTIAL** — thermodynamics, transport and both refinement studies done; the cumulative acceptance target is **INCONCLUSIVE** on this reference | `compositional/reference_tests.rs`, 10 `comp_reference_*` + 3 `comp_refinement_*`. Found and fixed three real defects: explicit wells, the injector connection law, and `COMPDAT`'s diameter. The milestone is **not** declared |
| C13–C14 | WASM, product integration, release | NOT STARTED | Gated on C12 |
| C15 | V1b immiscible water | NOT STARTED | Gated on C14 |

## 8. Completion records

### C12 — against an independent simulator (PARTIAL)

**The milestone `NATIVE-COMPOSITIONAL-READY` is NOT declared**, for one reason, stated up front:
the plan's cumulative-quantity acceptance target cannot be evaluated on this reference, because the
only cumulative it reports is a surface volume and the conversion is not one the two simulators are
known to share. That is an INCONCLUSIVE observable in the plan's own terms, not a failure, and
section "the observable that is inconclusive" below says what would settle it.

Everything else C12 asks for is done: the thermodynamic comparison, the transport comparison, the
timestep refinement and the grid refinement against a reference re-solved on each grid.

```text
C-task:                C12
Start / final commit:  f73468d / this commit
Oracle:                OPM flowexp_comp, built from release/2026.04/final (section 3b), running
                       OPM/opm-tests compositional/1D_COMP.DATA - a five-cell 1D CO2 flood in
                       CO2/methane/decane, which is ResSim's own V1 fluid, plus the same deck
                       refined to 10, 20 and 40 cells over the same 300 m
Fixtures:              opm/compositional/1d_comp/reference.json          (the deck's own grid)
                       opm/compositional/1d_comp/refined/n0{10,20,40}/   (the refinement ladder)
                       regenerated by run-1d-comp.sh and run-refinement.sh; refine_deck.py
                       asserts on every keyword it rewrites, so a change to the source deck fails
                       there rather than quietly producing a different case
Gates:                 bash scripts/validate-compositional.sh reference   (fixtures reproduce)
                       bash scripts/validate-compositional.sh all         (10 comp_reference_*)
                       bash scripts/validate-compositional.sh refinement  (3 comp_refinement_*,
                                                                           release, ~3 min)
Tests created:         10 comp_reference_*, 3 comp_refinement_*
```

#### What is compared

**Thermodynamics along the reference's own trajectory.** For every cell at every report step, the
reference's `(p, z)` is flashed by ResSim and the phase state, saturation and both phase
compositions are compared. 140 states, from the initial two-phase mixture through breakthrough to
essentially pure CO2. The fluid data was entered independently in the deck, the EOS and flash are a
different implementation in a different language, and nothing was tuned. Phase state agrees on all
140; worst saturation error 1.31e-7, worst composition error 5.55e-8 — both at the reference's
single-precision output resolution, so the two agree to the limit of what the fixture can express.

**ResSim's own trajectory.** ResSim builds the deck's case itself — transmissibility from DX/DY/DZ
and PERM, a Peaceman well index, the SGOF table, both wells with their deck controls — and advances
to the reference's own report times. This exercises upwinding, the well connection law and its
controls, the Newton lifecycle and the timestep controller against a simulator sharing none of that
code.

#### Measured, on the deck's own five-cell grid

```text
final state             0.003 bar   both simulators settled, 20.11 d
developed displacement  3.708 bar   from 2 d on; cell 3 at 14.11 d
startup transient       3.970 bar   cell 0 at 0.35 d
CO2 front               0.1327      in z_CO2, cell 4 at 17.11 d
cumulative injection    1.10%       FGIT at 20.11 d - see "inconclusive" below
Replay: cargo test --manifest-path src/lib/ressim/Cargo.toml --lib -- \
          comp_reference_transport_trajectory_tracks_opm \
          comp_reference_cumulative_injection_tracks_opm --nocapture
```

Both tests print these numbers, so the frozen bands can be rechecked rather than trusted.

#### Refinement

Timestep first, because a spatial claim is confounded without it. Ten cells, sub-step halved four
times:

```text
sub-step/d   final p/bar   worst p/bar   CO2 in place   cumulative
    0.1000         0.031         4.325         0.014%       2.234%
    0.0500         0.031         4.585         0.014%       2.581%
    0.0250         0.031         4.723         0.014%       2.781%
    0.0125         0.031         4.834         0.014%       2.883%
```

The settled state and the CO2 in place do not move with the sub-step at all. The transient and the
cumulative do, and they converge at first order — successive changes 0.347, 0.200, 0.102 on the
cumulative — which is what backward Euler gives. **0.05 d, the sub-step the comparison uses, is on
the converged part of that curve**, so the disagreement reported above is not the timestep
controller.

Then the grid, each reference re-solved on its own grid, at that sub-step:

```text
cells   final p/bar   worst p/bar   CO2 in place   cumulative
    5         0.003         3.970         0.002%       1.098%
   10         0.031         4.585         0.014%       2.581%
   20         0.028         5.654         0.018%       4.223%
   40         0.024         6.448         0.016%       5.122%
Replay: bash scripts/validate-compositional.sh refinement
```

**The settled pressure field agrees to 0.03 bar and the CO2 the grid holds to 0.02%, at every
resolution, with no drift.** Two independent implementations of compositional transport put the
same material in the same places. The worst pointwise pressure difference grows slightly with
refinement, which is expected: a finer grid resolves a sharper front, and a sharper front turns a
small difference in arrival time into a larger pointwise difference.

#### The observable that is inconclusive

Cumulative injection compared through `FGIT` disagrees by 1.1% at five cells and 5.1% at forty, and
**grows monotonically with refinement**. The plan's acceptance target for cumulative quantities is
1%, so this matters.

It is not the flow. Two observables of the same flow cannot disagree by two orders of magnitude —
CO2 in place stays under 0.02% across the same ladder. What is not shared is the accounting.
`FGIT` is a **surface volume**, and converting it to moles needs the reference's own surface molar
volume for the injected stream. The fixture cannot supply it: `GAS_DEN` and `OIL_DEN` come back
identically zero from `flowexp_comp`. The conversion therefore has to use ResSim's surface flash,
which is exactly what `comp_reference_surface_phase_label_is_pressure_blind` shows is unreliable at
1 bar. After breakthrough the injected CO2 passes straight through the grid, so throughput is
invisible in the in-place amount and the only record of it is each simulator's own bookkeeping.

The plan says a state/units mismatch is **INCONCLUSIVE, not a candidate failure**, and that is the
verdict recorded. `comp_refinement_surface_cumulative_is_inconclusive_not_a_failure` asserts the
argument rather than the conclusion: it fails if the conversion-free observable stops being small,
or if the two stop disagreeing by an order of magnitude, because either would mean this explanation
no longer holds.

**What would settle it:** a reference that writes its surface densities, or a summary vector in
moles.

#### Three defects the comparison found, that no unit test could have

**1. Wells were applied as an explicitly evaluated source.** A BHP well is strong negative feedback
— as a cell pressurizes, injection into it must fall — and a source held fixed over a step has no
feedback at all. Cell 0 reached 238 bar against a 150 bar injector and then oscillated by ±60 bar
between report times. Every well unit test imposes its own source, so every one of them passed.
Wells are now unknowns of the assembly: `assemble` takes them, contributes their residual and
Jacobian, and reports their rates through the Newton and timestep lifecycle.

**2. The injector used the injected stream's own mobility.** OPM's `StandardWell` uses the
perforation **cell's** total mobility, and while that cell is still unswept, pure supercritical CO2
is about four times more mobile than the mixture it is displacing. Measured as a 27% excess across
the interval where the injector hands over from rate to BHP control. The connection law was changed
to OPM's; `docs/COMPOSITIONAL_WELL_DESIGN.md` records the old rule, why it was defensible, and why
it is wrong. This was only visible because the refinement study forced a look at where the
cumulative discrepancy accrued in time — at the final state the perforation cell is pure CO2 and
the two laws agree to 0.4%, so a steady-state check would have cleared it.

**3. `COMPDAT` item 9 is a DIAMETER.** The driver read `0.0151` as the wellbore radius. The error
enters through `ln(r_eq / r_w)`, so it does not divide out with cell size: the well index came out
12% high at 10 cells and 15% high at 40, and the discrepancy *grew* under grid refinement. This is
a deck-semantics defect in the C12 driver, not in the engine, but it was indistinguishable from an
engine defect until it was found.

A fourth, smaller one: the reference's `TIME` vector is single precision, so the report time that
should be 0.11 arrives as 0.10999999940. The driver's well-opening tolerance of 1e-12 said the
wells were still shut at the moment they should have opened, and they stayed shut for a whole
sub-step. It showed up as a cumulative error that *grew* as the sub-step was refined — the
signature of a fixed amount of missing time being resolved away rather than of a converging
discretisation.

#### Driver decisions that mattered

```text
* the deck declares its wells AFTER four TSTEPs, so nothing flows for the first 0.11 days.
  Opening them at t = 0 put ResSim a whole displacement ahead before the comparison started
* the injector is SURFACE-RATE limited, not pure BHP. WBHP:INJ reads 135.29 bar at the first
  reported time and only later sits at 150. Pure BHP over-injects early - 22 bar of disagreement
* the surface rate targets SurfacePhase::Total, not Vapour, because Li's labelling is
  pressure-blind and calls surface CO2 a liquid. Asserted by
  comp_reference_surface_phase_label_is_pressure_blind; C13's reporting must name what it means
  by surface gas rather than inherit this label
* Newton tolerance 1e-7 rather than the default 1e-8. Late in the displacement methane and decane
  sit at ~1e-5 of a cell's inventory and C8's per-component scaling divides their balances by
  those tiny inventories; the solve reaches 1.8e-8 and stalls, bounded by the flash's own 1e-11
  equilibrium tolerance. A property of this case, not a default worth changing
* the deck's fluid is NOT the pinned fluid - it carries more precise critical properties than
  OPM's hard-coded ThreeComponentFluidSystem. The specification is built from THE DECK'S numbers,
  and a test asserts the two still differ so the comparison cannot quietly stop being independent
* OIL_DEN, GAS_DEN, OIL_VISC, GAS_VISC, FPR and every block summary vector come back identically
  zero from this simulator. The extractor prints a note for any such vector, so a column of zeros
  is never read as agreement
```

#### Remaining for C12

```text
* the cumulative acceptance target, which needs a reference that reports it in moles or reports
  its surface densities. Until then the plan's 1% is neither met nor failed - it is unevaluated
* a published benchmark. The plan lists this as an optional later expansion (C12 item 4) and it
  is not a blocker, but 1D_COMP is one case on one fluid
```

The per-observable bands frozen above are for **this** case at **this** resolution, and the tests
that carry them print their measurements so they can be rechecked rather than trusted.

### C11 — compositional wells

Design note written before the implementation, as the plan requires:
[`COMPOSITIONAL_WELL_DESIGN.md`](COMPOSITIONAL_WELL_DESIGN.md).

```text
C-task:                C11 (one completion; multiple completions are a separate commit)
Start / final commit:  d1ea1e4 / this commit
Provenance:            DERIVED FROM FIRST PRINCIPLES plus this repository's Peaceman geometry.
                       C11's named reference - OPM's CompWellModel and CompWellFlash.hpp -
                       lives in opm-simulators/flowexperimental/comp/, which Debian does not
                       package (plan correction 3). No OPM well header was read, so **no
                       claim of agreement with OPM's well model is made or gated**
Geometry reuse:        the geometric part of well_control.rs's productivity index, with total
                       mobility factored back out so one connection can carry two phases at
                       different mobilities. Same Darcy constant, same Peaceman form
Sign convention:       positive source = moles ENTERING the cell, matching C8's residual
Injection rule:        SUPERSEDED BY C12. As implemented here it was the injected stream's own
                       total mobility at connection conditions - a defensible modelling choice
                       that assumes the near-wellbore region is occupied by the injected fluid,
                       and one that avoids the failure the plan names (a phase evaluated at a
                       saturation belonging to a different fluid). C12 measured it against
                       flowexp_comp and it over-injects by about 4x while the perforation cell
                       is still unswept. The rule is now the perforation CELL'S total mobility,
                       which is OPM's law and which also cannot vanish, being a sum over the
                       phases present. See the C12 record and COMPOSITIONAL_WELL_DESIGN.md
Wellbore flash:        NOT required by this formulation, and the design note says why: a
                       single completion with a prescribed injection composition has no mixing
                       to resolve. A multi-completion well does, which is one reason that is a
                       separate commit rather than a loop
Controls:              BHP; total molar rate with a BHP limit; surface volumetric rate through
                       C5's single-stage flash. The last is explicitly NOT a reservoir rate and
                       not a black-oil RESV target
Tests created:         20 comp_well_*
Commands:              bash scripts/validate-compositional.sh all
                       bash scripts/validate-solver-coverage.sh all
Results:               233 comp_ tests pass; compositional gate green; solver coverage 38/38
Out of scope here:     multiple completions, crossflow, wellbore mixing and friction,
                       multisegment wells, separator trains beyond C5's single stage
Completed gate:        C11
Next permitted task:   C12 - which is BLOCKED (section 3b)
```

### C10 — linear solve, Newton and the timestep lifecycle

```text
C-task:                C10 (direct linear path; iterative adapter deferred)
Start / final commit:  095147e / this commit
Component/phase count: N=2 and N=3; two-phase and both single-phase regimes
Linear solve:          dense Gaussian elimination with partial pivoting, on the row-scaled
                       system. This is the plan's step 1 - the correction-quality oracle a
                       component-aware block-ILU/CPR adapter would be measured against. That
                       adapter is NOT implemented: its whole value is the comparison, and
                       building it before the oracle exists would invert the argument
Update policy:         fraction-to-boundary at 0.99, with ONE global scale factor. A per-cell
                       factor would change the Newton direction rather than its length, and
                       the Jacobian was computed for the direction. The bound includes the
                       dependent z_(N-1), whose direction is -sum(dz_k) and which has no
                       column to inspect. Nothing is clamped or renormalized after the step
Zero components:       a positive direction on an empty component is unconstrained, so an
                       absent component can be injected into existence. A materially negative
                       one is a named failure, not a clamp - it means that component's
                       residual was not what it should have been
Changed interfaces:    new `compositional::newton` and `compositional::timestep`.
                       `fluid::derivatives` fixed - see below
Tests created:         20 - comp_newton_* 10, comp_rollback_* 10, plus one C4 regression
Commands:              bash scripts/validate-compositional.sh all
                       bash scripts/validate-solver-coverage.sh all
Results:               203 comp_ tests pass; compositional gate green; solver coverage 38/38
Defect found by C10:   `fluid::derivatives` computed ln K as ln(y_i / x_i), which is 0/0 for an
                       exactly zero component - so a cell with an absent component produced a
                       NaN before any Newton step was taken. Equilibrium ratios now come from
                       the fugacity coefficients, which are the same number wherever both are
                       defined and are defined where the ratio is not. C4 gained a regression
                       test; the C3 flash was already correct, because it skips absent
                       components rather than dividing by them
Completed gate:        C10 (direct path)
Next permitted task:   C11, or C9's gravity subtask, or C10's iterative adapter
```

### C9 — component face flux and global assembly

```text
C-task:                C9 (gravity subtask NOT done - see below)
Start / final commit:  e1f4396 / this commit
Component/phase count: N=2 and N=3, AD instantiations through N=4; two-phase and both
                       single-phase upstream regimes
Source equation:       dphi   = p_i - p_j                                        [bar]
                       q_P    = geom_t (kr_P / mu_P)|upstream dphi               [m3/day]
                       flux_i = sum_P q_P c_P|upstream x_(P,i)|upstream          [mol/day]
                       geom_t is DARCY_METRIC_FACTOR * geometric_transmissibility, the same
                       quantity fim/flux.rs receives - reused, not re-derived
Upwind convention:     branch on the value of dphi, `dphi >= 0` selects the first cell, then
                       frozen for the whole Jacobian evaluation. Identical to fim/flux.rs
Changed interfaces:    new `compositional::flux` and `compositional::assembly`
Tests created:         24 - comp_flux_* 15, comp_assembly_* 9
Commands:              bash scripts/validate-compositional.sh all
                       bash scripts/validate-solver-coverage.sh all
Results:               183 comp_ tests pass; compositional gate green; solver coverage 38/38
Worst errors:          face Jacobian vs FD < 1e-4; assembled Jacobian vs an independent
                       numerical one < 1e-5 column-scaled; internal-face cancellation exact
Modelling assumption:  straight-line hydrocarbon relative permeability. Declared, not sourced.
                       See section 6, item 2 - this needs a decision before C12/C13
Gravity subtask:       DONE, in its own commit as the plan requires, and gated on a
                       single-phase hydrostatic equilibrium before any multiphase case. The
                       oracle is analytic - dp/dz = rho g - not a fixture. Head constant and
                       sign convention are fim/flux.rs::gravity_head_generic's, reproduced, so
                       the two models agree about which way is down. Off by default; V1 starts
                       at zero gravity
Still zero in V1:      hydrocarbon capillary pressure. Unequal phase pressures need a
                       separately derived equilibrium contract and cannot be enabled by
                       passing an old flag
Completed gate:        C9 (non-gravity)
Next permitted task:   C9 gravity subtask, or C10
```

### C8 — cell inventory, accumulation and scaling

```text
C-task:                C8
Start / final commit:  6f22cdb / this commit
Prerequisites:         THERMO-READY (C6), C7, FIM-COMPOSITIONAL-SEAM-READY - all satisfied
Component/phase count: N=2 and N=3, with AD instantiations through N=4; two-phase,
                       single-liquid and single-vapour regimes all covered by the test fixture
Source equation:       n_i = PV(p) (S_L cL x_i + S_V cV y_i)     [mol]
                       R_i = n_i(new) - n_i(previous) - dt source_i
                       PV(p) = PV_ref exp(c_rock (p - p_ref)), the existing sourced relation
Units:                 mol and mol/day throughout; dt in days; the pressure primary is bar, so
                       the Jacobian's pressure column is per bar and the conversion to the
                       EOS's pascals happens once, at the boundary
Changed interfaces:    new `compositional::accumulation`. Nothing existing changed
Tests created:         19 - comp_accumulation_* 12, comp_scaling_* 7
Commands:              bash scripts/validate-compositional.sh all
                       bash scripts/validate-solver-coverage.sh all
Results:               159 comp_ tests pass; compositional gate green; solver coverage 38/38
Worst errors:          inventory identity 1e-12; Jacobian vs FD < 1e-5; a stationary closed
                       cell is exactly zero, not approximately
Equation count:        N component balances against N primaries [p, z_0..z_(N-2)] - the
                       plan's "exactly as many independent equations as primaries", asserted
                       by comp_layout_has_as_many_equations_as_primaries
Scaling:               per-component row scales from the PREVIOUS accepted inventory, with a
                       floor at 1e-8 of the cell's total moles. Per-component rather than
                       cell-wide because a cell-wide scale lets a component holding 1e-4 of
                       the material stall at 20% error while reporting 1e-5; referenced to the
                       previous state because a scale that moved with the iterate would let a
                       step converge by shrinking its own denominator
Completed gate:        C8
Next permitted task:   C9
```

### C7 — component layout, state and the geometry boundary

```text
C-task:                C7
Start / final commit:  26bb4b1 / this commit
Component/phase count: N=2 and N=3 production layouts, plus a layout-only N=4 case
Consumed from F7:      FimLinearBlockLayout (fim/linear/mod.rs) as the shared linear metadata,
                       and fim::layout::CellPrimary::Pressure's local index as the CPR
                       pressure-column convention. Neither is re-derived
Changed interfaces:    new private module `compositional` with `layout` and `state`. The
                       black-oil CELL_BLOCK_SIZE, offsets and layout are untouched, and a test
                       asserts it
Tests created:         30 - comp_layout_* 12, comp_state_* 18
Commands:              bash scripts/validate-compositional.sh all
                       bash scripts/validate-solver-coverage.sh all
Results:               140 comp_ tests pass; compositional gate green; solver coverage 38/38;
                       fmt clean; wasm32 compiles; no new build warnings
Design decisions:      - a cell stores p and z_0..z_(N-2); the dependent component is derived,
                         never stored, so sum z = 1 cannot be broken by an update
                       - accepted state and Newton trial are separate types and the only route
                         between them is commit(), so "a rejected step changed nothing" is a
                         property rather than a claim
                       - the flash cache keys on exact bit patterns plus the state version, so
                         a commit invalidates everything without walking the cache
                       - RockView borrows the pore-volume array and the two rock constants, and
                         nothing else; no ReservoirSimulator reaches the physics
Completed gate:        C7
Next permitted task:   C8 (prerequisites THERMO-READY and FIM-COMPOSITIONAL-SEAM-READY are both
                       satisfied)
```

### C6 — THERMO-READY

**Milestone declared.** The standalone thermodynamics is admitted for the pinned fluids over
10–500 bar at 423.15 K (and 333.15 K, where the fixture samples it). **This is a thermodynamics
milestone and nothing more**: no transport, no wells, no browser exposure, and the compositional
*trajectory* oracle remains blocked (§3b), so nothing here supports a claim about a flow result.

```text
C-task:                C6
Start / final commit:  49d6da0 / this commit
Component/phase count: N=2 and N=3, both phases and both single-phase branches
Gate:                  bash scripts/validate-compositional.sh all
                         thermo  - 110 tests across 10 filters, each with a floor count
                         fixture - regenerates the C0 oracle and diffs it byte for byte
                         wasm    - cargo check --target wasm32-unknown-unknown
Tests:                 110 total. comp_spec_ 19, comp_units_ 9, comp_eos_ 20,
                       comp_stability_ 7, comp_rr_ 6, comp_flash_ 13, comp_derivatives_ 11,
                       comp_transport_ 11, comp_surface_ 10, comp_domain_ 4
Sweep:                 7 620 deterministic samples, 10-600 bar (past the declared envelope on
                       purpose), over both simplices. Every sample classified; zero failures
Worst errors:          z reconstruction 1.1e-14; dx column closure 2.9e-16; trace-component
                       conservation < 1e-8 down to z_i = 1e-10
Conditioning:          min equilibrium-Jacobian pivot 4.8e-4; min |dP/dZ| > 1e-3
Native/WASM:           the crate compiles for wasm32-unknown-unknown. No dependency blocks it
External parity:       flash parity MET against the OPM PTFlash harness (section 3a)
                       trajectory parity BLOCKED - no compositional executable (section 3b)
Defect found by C6:    root selection counted root *multiplicity* rather than *admissibility*.
                       At 500-600 bar and 96-98 mol% methane the cubic has three real roots of
                       which two are negative: one fluid state, and two non-states. Selecting
                       the smallest for the liquid label returned a Z below the covolume and
                       failed the stability test on a perfectly well-defined state. Now the
                       largest and smallest *admissible* roots are selected, and a single
                       admissible root serves both labels
Completed gate:        C6 / THERMO-READY
Next permitted task:   C7
```

### C5 — viscosity, saturations and the surface separation

```text
C-task:                C5
Start / final commit:  ec5c399 / this commit
Component/phase count: N=2 and N=3; both phases, and the single-phase and zero-flow limits
Source equations:      viscositymodels/LBC.hpp - Lohrenz, Bray & Clark, JPT 16.10 (1964), with
                       that header's correction of the paper's -0.40758 typo to -0.040758
                       CompositionalConfig.hpp - standard_temperature 288.71 K,
                       standard_pressure 1 atm; Units.hpp - atm = 101325 Pa
Changed interfaces:    new `fluid::transport`; `fluid::flash` gains the extended
                       (negative-flash) Rachford-Rice window
Tests created:         17 - comp_transport_* and comp_surface_*
Commands:              cargo test --manifest-path src/lib/ressim/Cargo.toml comp_
                       cargo fmt --manifest-path src/lib/ressim/Cargo.toml -- --check
                       cargo check --manifest-path src/lib/ressim/Cargo.toml \
                           --target wasm32-unknown-unknown
                       bash scripts/validate-solver-coverage.sh all
Results:               106/106 comp_ pass; fmt clean; wasm32 compiles; solver coverage 38/38
Worst errors:          LBC 1e-13 vs the fixture at OPM's own density; saturation
                       reconstruction 1e-14; surface component conservation 1e-9
Native/WASM coverage:  both
Open decision closed:  surface conditions (section 6, item 1)
Flash change:          the iteration now runs on Whitson & Michelsen's extended window. A
                       nearly pure n-decane stream with 0.2 mol% light ends at surface
                       conditions is declared unstable by the stability test and has no
                       Rachford-Rice root in [0, 1]; the extended window resolves it as a
                       single-phase liquid, which is a determination rather than a failure
Completed gate:        C5
Next permitted task:   C6
```

### C4 — equilibrium and property derivatives

```text
C-task:                C4
Start / final commit:  eb50726 / this commit
Component/phase count: N=2, 3 and 4 AD instantiations; two-phase and both single-phase branches
Source equations:      PTFlash.hpp::updateDerivatives_ - implicit differentiation of the
                       converged equilibrium, not of the iteration
                       F = [ln K_i - ln phi_i^L + ln phi_i^V ; sum(v_i - x_i)]
                       y = [ln K_0..ln K_(N-1), beta], u = [p_Pa, z_0..z_(N-2)]
                       dy/du = -F_y^{-1} F_u by a pivoted dense solve, never an inverse
Changed interfaces:    new `fluid::derivatives`; `fluid::eos` mixing rule and fugacity
                       coefficients generified over the `Scalar` trait, with the f64 path
                       value-identical (the C2 tests are unchanged and still pass);
                       `crate::fim::ad` moved to `crate::ad` in its own commit
Tests created:         11 comp_derivatives_* in fluid/derivative_tests.rs
Commands:              cargo test --manifest-path src/lib/ressim/Cargo.toml comp_
                       cargo fmt --manifest-path src/lib/ressim/Cargo.toml -- --check
                       cargo check --manifest-path src/lib/ressim/Cargo.toml \
                           --target wasm32-unknown-unknown
                       bash scripts/validate-solver-coverage.sh all
Results:               82/82 comp_ pass; fmt clean; wasm32 compiles; solver coverage 38/38
Worst errors:          dbeta/du and dx,dy/du 8.7e-10 vs OPM; molar density 3.4e-10;
                       composition-column closure 1e-12; dependent-z derivative exact
Native/WASM coverage:  both
Unsupported states:    near-repeated cubic roots (DegenerateRoot) and a singular equilibrium
                       Jacobian (SingularEquilibriumJacobian) are typed errors carrying the
                       offending quantity. No fixture state reaches either
Completed gate:        C4
Next permitted task:   C5
```

### C2 — Peng–Robinson mixture EOS and single-phase properties

```text
C-task:                C2
Start / final commit:  cbf2142 / this commit
Component/phase count: N=2 and N=3; single-phase branches only - no equilibrium is computed
Source equations:      eos/PRParams.hpp        - Omega_A, Omega_B, m1, m2, f(w)
                       eos/CubicEOSParams.hpp  - A_i, B_i, a_ij mixing rule, mixture A and B
                       eos/CubicEOS.hpp        - cubic coefficients, root labelling, ln phi
                       common/PolynomialUtils.hpp - trigonometric/hyperbolic cubic solver
                       all libopm-common-dev 2026.04-1~noble
Changed interfaces:    new `fluid::eos`; fixture schema /1 -> /2 (adds `eos_states`); no
                       existing interface, public API or WASM surface changed
Tests created:         22 comp_eos_* in fluid/eos_tests.rs
Commands:              cargo test --manifest-path src/lib/ressim/Cargo.toml comp_
                       cargo fmt --manifest-path src/lib/ressim/Cargo.toml -- --check
                       cargo check --manifest-path src/lib/ressim/Cargo.toml \
                           --target wasm32-unknown-unknown
                       bash tools/opm_compositional/generate.sh --check
                       bash scripts/validate-solver-coverage.sh shared
Results:               48/48 comp_ pass; fmt clean; wasm32 compiles; fixture reproduces
                       byte-for-byte; shared gate 17/17
Worst errors:          A/B 1e-13, Z 1e-12, phi 1e-11, density 1e-7 after the exact R correction,
                       flash-free states 1e-11. See the acceptance matrix in section 5
Native/WASM coverage:  both
Fixture change:        the original 47 flashed states contain no three-root cubic, so 21
                       flash-free EOS states were added to satisfy C2's multi-root requirement
Deliberate divergence: OPM's phi clamp, mole-fraction clamp and molar-volume floor are NOT
                       reproduced; a test confirms no fixture state approaches any of them
Completed gate:        C2
Next permitted task:   C3
```

### C1 — fluid specification and unit-safe input

```text
C-task:                C1
Start / final commit:  2483fb3 / this commit
Component/phase count: N=2 and N=3 pinned; the type admits N=4 for C7's layout-only test
Source equations:      none - C1 implements no physics. Component data is transcribed from
                       /usr/include/opm/material/components/{C1,C10,SimpleCO2}.hpp and is
                       checked against opm/compositional/ptflash_fixtures.json by a test
Changed interfaces:    new private module `fluid`; no existing interface changed; the crate's
                       public API and the WASM surface are untouched
Tests created:         26 - comp_spec_* (18, fluid/tests.rs) and comp_units_* (8, units.rs)
Commands:              cargo test --manifest-path src/lib/ressim/Cargo.toml comp_
                       cargo fmt --manifest-path src/lib/ressim/Cargo.toml -- --check
                       cargo check --manifest-path src/lib/ressim/Cargo.toml \
                           --target wasm32-unknown-unknown
                       bash scripts/validate-solver-coverage.sh shared
Results:               26/26 comp_ pass; fmt clean; wasm32 target compiles; shared gate 17/17
Worst errors:          n/a - C1 has no numerical output
Native/WASM coverage:  both; the module has no wasm-bindgen or browser dependency
Missing oracle:        surface conditions are unpinned and therefore Optional (section 6)
Completed gate:        C1
Next permitted task:   C2
```

### C0 — fluid, oracle and acceptance contract

```text
C-task:                C0
Start / final commit:  f5838eb / recorded in opm/compositional/manifest.json
Component/phase count: N=2 (C1/C10) and N=3 (CO2/C1/C10); 1 or 2 hydrocarbon phases
Source equations:      PR EOS - /usr/include/opm/material/eos/{PRParams,CubicEOS}.hpp
                       PTFlash - .../constraintsolvers/PTFlash.hpp
                       LBC     - .../viscositymodels/LBC.hpp
                       all from libopm-common-dev 2026.04-1~noble; no git checkout exists
Fluid checksum:        opm/compositional/manifest.json, field "sha256"
Unit convention:       SI in the kernel; see section 2
Changed interfaces:    none - no Rust source was modified by C0
Tests created:         none - C0 creates no Rust tests by design; C1 creates the first
Commands:              bash tools/opm_compositional/generate.sh
                       bash tools/opm_compositional/generate.sh --check
Worst oracle errors:   equilibrium 5.1e-10 (binary_p100); z reconstruction 1.1e-16;
                       normalization 2.2e-16; derivative closure 6.1e-16
Native/WASM coverage:  n/a - C0 produces no shipped code
Missing oracle:        compositional trajectory executable (section 3b). C12 BLOCKED
Completed gate:        C0
Next permitted task:   C1
```
