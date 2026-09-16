# Compositional validation

Owning document for the compositional model's fluid dataset, numerical acceptance contract,
oracles and gate results. Created by task **C0** of the
[compositional fluid execution plan](COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md), whose
[2026-09-16 corrections](COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md#plan-corrections--2026-09-16)
pin the dataset and oracle recorded here. Tracking issue:
[#29](https://github.com/sergeyfarin/ressim/issues/29).

**Nothing in this document claims a gate passes.** The gate table at the bottom records the state
of each one, and every entry is currently `NOT STARTED` or `BLOCKED` except C0 itself.

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
bash tools/opm_compositional/generate.sh          # regenerate
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

### 3b. Trajectory oracle — BLOCKED

There is no compositional flow executable on this machine. `libopm-simulators-bin` installs
`/usr/bin/flow` and nothing else, and `flow 2026.04` is the black-oil simulator. `flow_comp` lives
in `opm-simulators/flowexperimental/comp/`, which Debian does not package and which is therefore
also unreadable as source here.

**Consequence:** C12 cannot close, and no compositional trajectory claim may be made, until
someone builds that application. §3a does not substitute for it — a scalar flash oracle cannot
referee a flow trajectory. Independent invariants (closed-system inventory, timestep and grid
refinement, FD Jacobians) remain fully available and are what C8–C11 are gated on.

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
| Derivative closure | — | `sum_i d(x_i)/du_v` and `sum_i d(y_i)/du_v` worst 6.1e-16 (`binary_T333_p150`) | The oracle's derivatives satisfy the normalization identity to roundoff |
| Smooth property derivatives | ≤ 1e-4 relative, on a frozen nonzero derivative scale, over an FD step plateau | Deferred to C4 — needs the Rust implementation to compare against | Not yet |
| Tiny assembled Jacobian | ≤ 1e-5 scaled entrywise vs FD at smooth states | Deferred to C8/C9 | Not yet |
| Tiny direct linear oracle | ≤ 1e-10 full-system relative residual | Deferred to C10 | Not yet |
| Local conservative face exchange | Cancellation to roundoff | Deferred to C9 | Not yet |
| Global closed/source balance | ≤ 1e-8 relative accumulated mole error per component | Deferred to C9/C10 | Not yet |
| Refined external trajectories | ≤ 1% on cumulative quantities, per-observable tolerances in C12 | **Cannot be set** — no trajectory oracle (§3b) | Blocked |
| Newton acceptance | Derived and frozen in C8/C11 | Deferred | Not yet |

No existing black-oil benchmark tolerance is changed by any of this.

## 6. Open decisions this document owes

1. **Surface conditions are not pinned.** C5 needs an explicit `p_surface`, `T_surface` and a
   single declared surface flash stage. 1 atm / 15.56 °C is the obvious candidate but has not been
   sourced against anything in this repository's existing conventions, and the black-oil path's
   standard conditions must not be assumed to carry over. C5 must pin it before writing the
   surface flash, and record it here.
2. **The hydrocarbon relative permeability law is not pinned.** The plan is explicit that the
   existing water/oil curves are not implicitly a hydrocarbon liquid/vapour law. C9 and C12 need
   one, sourced. Nothing in C0–C5 depends on it.
3. **The first flow fixture is specified but not built.** 1D uniform column, no gravity or
   capillarity, fixed temperature, one injection and one production boundary, composition chosen
   to force a phase change inside the declared domain. It cannot be given an external reference
   (§3b), so its acceptance rests on invariants.
4. **The near-critical domain is unclassified.** C6 decides whether to declare it supported or to
   narrow the domain and reject those inputs. The current fixture deliberately does not probe it.

## 7. Gate status

| Task | Deliverable | Status | Evidence |
| --- | --- | --- | --- |
| C0 | Frozen scope, fluid dataset, oracles, acceptance contract | **COMPLETE** | This document; `opm/compositional/`; `tools/opm_compositional/` |
| C1 | Fluid specification and unit-safe input | **COMPLETE** | `src/lib/ressim/src/fluid/{specification,units,pinned}.rs`; 26 `comp_spec_*` / `comp_units_*` tests |
| C2 | PR mixture EOS and single-phase properties | **COMPLETE** | `src/lib/ressim/src/fluid/eos.rs`; 22 `comp_eos_*` tests |
| C3 | Stability and scalar PT flash | **COMPLETE** | `fluid/{stability,flash}.rs`; 7 `comp_stability_*`, 12 `comp_flash_*`, 4 `comp_rr_*`; 47/47 states match OPM |
| C4 | Equilibrium and property derivatives | **COMPLETE** | `fluid/derivatives.rs`; 11 `comp_derivatives_*` tests |
| C5 | Transport properties and surface flash | NOT STARTED | — |
| C6 | THERMO-READY | NOT STARTED | External flash parity available (§3a); trajectory parity blocked |
| C7 | Component layout and state | NOT STARTED | Prerequisite `fim/layout.rs` available |
| C8–C11 | Accumulation, flux, Newton, wells | NOT STARTED | `FIM-COMPOSITIONAL-SEAM-READY` declared at `6be6d08` |
| C12 | NATIVE-COMPOSITIONAL-READY | **BLOCKED** | No compositional Flow executable (§3b) |
| C13–C14 | WASM, product integration, release | NOT STARTED | Gated on C12 |
| C15 | V1b immiscible water | NOT STARTED | Gated on C14 |

## 8. Completion records

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
