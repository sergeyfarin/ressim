# Compositional fluid and FIM integration execution plan

Date: 2026-09-14; **corrected 2026-09-16** (see [Plan corrections](#plan-corrections--2026-09-16)).
Planning base: `f9dd22e`; corrections verified on `f5838eb`. **No compositional implementation is
delivered by this document.** Execution tracking belongs to
[#29](https://github.com/sergeyfarin/ressim/issues/29); create linked child issues when starting
independent implementation stages. The [readiness assessment](COMPOSITIONAL_READINESS_ASSESSMENT_2026-09-14.md)
contains the audit and source hashes. The [FIM repair plan](FIM_REPAIR_EXECUTION_PLAN_2026-09-14.md)
defines the integration prerequisite.

## Plan corrections — 2026-09-16

Applied on branch `compositional-modelling`, base `f5838eb`. Every item below was verified
against this machine, not inferred from the 2026-09-14 planning session. The task list, scope
and acceptance matrix are **unchanged**; these corrections fix stale prerequisites, wrong source
paths and one unavailable dataset, and they upgrade one oracle from "contingent" to "available".

### 1. The FIM prerequisite is satisfied

`FIM-COMPOSITIONAL-SEAM-READY` was declared at `6be6d08`
([handoff](FIM_REPAIR_HANDOFF_2026-09-15.md) line 13); `FIM-REPAIR-READY` at `57ecb8e`.
[#27](https://github.com/sergeyfarin/ressim/issues/27),
[#28](https://github.com/sergeyfarin/ressim/issues/28) and
[#13](https://github.com/sergeyfarin/ressim/issues/13) are **closed** — both well findings were
stale test oracles, not production defects. The gate this plan names for C8-and-later is
therefore open, and C7 may consume `fim/layout.rs` (`CELL_BLOCK_SIZE`, `CellPrimary`,
`CellEquation`), `FimLinearBlockLayout` / `FimLinearSolveReport` (`fim/linear/mod.rs`) and
`EquationScaling` / `EquationFamilyPeaks` (`fim/scaling.rs`) as delivered interfaces.
`fim/ad.rs` referenced by C4 exists.

### 2. OPM is installed as packages, not as git checkouts

The readiness assessment pins `OPM/opm-simulators`, `OPM/opm-common` and `OPM/opm-models`
working copies at three commit hashes. **Those checkouts do not exist on this machine.** What
exists is the Ubuntu noble OPM packaging:

| Package | Version | Supplies |
| --- | --- | --- |
| `libopm-common-dev:amd64` | `2026.04-1~noble` | `/usr/include/opm/material/**` — `eos/`, `constraintsolvers/PTFlash.hpp`, `fluidsystems/`, `viscositymodels/LBC.hpp`, `components/` |
| `libopm-simulators-dev:amd64` | `2026.04-1~noble` | `/usr/include/opm/models/ptflash/*.hh` |
| `libopm-simulators-bin` | `2026.04-1~noble` | `/usr/bin/flow` only |
| `libopm-grid-dev:amd64` | `2026.04-1~noble` | grid headers |

Read every C0/C3/C11 "OPM source" reference at `/usr/include/opm/...`, and record provenance as
the package version plus `dpkg -S <header>`, not a git hash. The three hashes in the assessment
remain valid as upstream citations; they are not reproducible artifacts here.

### 3. `flowexperimental/comp/` is not packaged — C11's reading list is unavailable

`CompWellModel`, `wells/CompWellFlash.hpp` and `flow_comp.cpp/.hpp` are an unpackaged
experimental application in `opm-simulators`. `find /usr/include/opm -path '*comp*'` returns no
well headers. C11 therefore **cannot** open its named reference in this environment. C11 must
either (a) fetch upstream `opm-simulators` source over the network under the normal approval
rules, or (b) derive the compositional well contract from first principles and the installed
`opm/models/ptflash/` residual headers, and say so. It must not paraphrase a header it has not
read. This does not block C0–C10.

### 4. Fluid dataset: adopt OPM's installed CO2/C1/C10 system, not C1/nC4/nC10

The plan suggested a methane/n-butane/n-decane ternary "only after C0 pins a complete sourced
dataset". No complete sourced n-butane dataset exists on this machine, and the plan forbids
inventing critical properties. `/usr/include/opm/material/fluidsystems/ThreeComponentFluidSystem.hh`
(installed, SINTEF 2022, GPL-2+) instead pins a **complete** ternary — CO2 / C1 / C10 — with
every quantity C2/C3/C5 need, including the critical volumes LBC requires and an explicit
`interactionCoefficient() == 0.0` for all pairs. It is also the fluid OPM's own compositional
path is written around, which makes the harness oracle and the model agree by construction.

**V1 fluid is therefore CO2/C1/C10** (ternary) with a **C1/C10 binary** reduction for N=2.
Full values and provenance live in `COMPOSITIONAL_VALIDATION.md`. CO2 here is a PR-EOS
hydrocarbon-system component at fixed temperature; this is **not** aqueous CO2, and the plan's
exclusion of brine/aqueous-CO2 thermodynamics is unchanged and still binding.

### 5. The thermodynamic oracle is available now; the trajectory oracle is still blocked

C0 treats a runnable OPM reference as contingent ("prepare a pinned build recipe"). For
**thermodynamics that contingency is resolved**: OPM's PTFlash/CubicEOS/LBC stack is
header-only and compiles against the installed packages with no OPM build and no MPI:

```bash
g++ -std=c++20 -I/usr/include -I/usr/include/dune harness.cpp -o harness
```

`-std=c++20` is required — `opm/material/Constants.hpp` and `PolynomialUtils.hpp` use
`std::numbers`, and `dune/common/std/algorithm.hh` requires three-way comparison. C17 fails.

The **trajectory** oracle remains **BLOCKED**, exactly as
[#29](https://github.com/sergeyfarin/ressim/issues/29) states: `libopm-simulators-bin` ships
`/usr/bin/flow` and nothing else, and `flow` 2026.04 is the black-oil simulator. C12 cannot be
closed from this environment without building the experimental compositional application.
These two oracle capabilities stay separately tracked, as the plan already requires.

### 6. Pinned conventions read out of the installed source

Recorded here because three of them are exactly the confusions the plan warns about.

| Convention | Installed source | Value |
| --- | --- | --- |
| Phase-split variable | `PTFlash.hpp` `flash_solve_scalar_`, `rachfordRice_g_` | OPM solves for **`L` = liquid mole fraction**. This plan's `beta` is the **vapor** fraction: `beta = 1 - L`. Never pass one where the other is meant. |
| EOS variant | `CompositionalConfig::EOSType` | `{PR, PRCORR, RK, SRK, ZJ}`. **V1 pins `PR`** (unmodified). |
| PR constants | `eos/PRParams.hpp` | `OmegaA = 0.457235529 * (1 + f(w)(1 - sqrt(Tr)))^2`, `OmegaB = 0.077796074`, `m1 = 1 + sqrt(2)`, `m2 = 1 - sqrt(2)`, `f(w) = 0.37464 + 1.54226 w - 0.26992 w^2`. The `PRCORR` branch swaps `f(w)` for a quartic when `w > 0.49`; C10's acentric factor is 0.488, just under that threshold, so PR and PRCORR agree on this fluid. Do not treat that coincidence as permission to leave the variant unpinned. |
| Fugacity coefficient | `eos/CubicEOS.hpp` | `ln phi_i = -ln(Z - B) + (Bi/B)(Z - 1) + [ln((Z + m2 B)/(Z + m1 B)) * A / ((m1 - m2) B)] * ((2/A) sum_j A_ij y_j - Bi/B)`. OPM then clamps `phi` into `[1e-10, 1e10]`; the Rust port **must not** copy that clamp into the delivered model — plan rule 6 forbids clamping an invalid result — but a fixture generated at a clamped state is unusable and must be excluded by C0, not matched. |
| Root selection | `eos/CubicEOS.hpp::computeMolarVolume` | Three real roots: largest `Z` for the gas branch, smallest for the liquid branch; one real root: that root for both. Root *labelling*, not stability — the plan's C2 rule stands. |

### 7. Consequent edits already applied to the task text

C0 "Read", C3, C5 and C11 source paths now point at `/usr/include/opm/...`; C0's dataset step
names the CO2/C1/C10 system; C0's oracle step records the verified harness command. Nothing
else in C0–C15, the acceptance matrix or the stop conditions was changed.

## Target and explicit scope

Deliver **V1: isothermal hydrocarbon compositional flow**, with a Peng–Robinson EOS, two supported
component counts (2 and 3), one or two hydrocarbon phases, phase appearance/disappearance,
composition-dependent density/viscosity, component-conservative fully implicit transport,
specified-composition injection and component production. The V1 fluid is the **CO2/C1/C10 ternary** pinned by correction 4 from OPM's installed
`ThreeComponentFluidSystem`, with a **C1/C10 binary** as the N=2 reduction. (The original text
suggested methane/n-butane/n-decane; no complete sourced n-butane dataset is available here.)
Do not invent critical properties or binary interaction values.

V1 uses Cartesian geometry, a single fluid region, constant prescribed temperature, no water,
no hydrocarbon capillary pressure, initially zero gravity, and no reactions/diffusion. Gravity
is enabled only in its own validated C9 subtask. C15 adds immiscible water as **V1b**. Public V1
must reject unsupported options explicitly rather than silently ignoring them.

This is compositional FIM, not a black-oil approximation with additional gas labels. Do not
reuse `Rs` as composition or surface black-oil volume as component inventory. Existing IMPES
and black-oil scenarios remain separate supported paths. No compositional IMPES is planned.

Deferred: energy equation, brine/aqueous CO2 dissolution, reactions, diffusion, geomechanics,
multiple EOS regions, arbitrary component counts, near-critical production guarantees,
multisegment wells and a full separator train. A single declared surface flash is required for
V1 surface rates. If the user's consuming case becomes CO2/brine storage, stop this scope and
write the aqueous fluid contract; hydrocarbon PR alone is insufficient.

## Rules for a less experienced executing model

1. Execute one C-task at a time. New file names and `comp_*` test names below are **proposed**;
   create them in the owning task before running their filters. A zero-test run is a failure.
2. Read the repository instructions and validation/engine skills every session. Read the FIM
   skill for global solver work, OPM skill for fixtures, frontend skill before C13, and
   add-scenario skill before admitting a catalog case. Use pnpm and uv conventions.
3. Preserve unrelated changes. Record start/end commits and exact commands. Commit validated,
   bounded stages. No push, deployment or new dependency by default. Reuse existing numerical
   libraries; document why an additional dependency would be essential before adding one.
4. Equations, units, independent oracle and tolerances come **before** the numerical code.
   A design checkpoint is satisfied by a reviewed source-backed contract and its tests, not
   by guessing which plausible formula to use. Escalate an unresolved numerical choice with
   concrete alternatives and evidence; do not repeatedly tune until a reference happens to match.
5. Do not introduce a generic framework for all future physics. Isolate thermodynamics first;
   generalize only solver interfaces actually consumed by this model.
6. Never silently normalize invalid input, clamp component loss away, return a cached state
   after flash failure, or accept unconverged Newton/flash results. Return a typed error and
   preserve the last accepted state. A valid single-phase flash is success, not a failure.
7. Existing benchmark tolerances are unchanged. New thresholds listed below are proposed
   admission targets; C0 must confirm they are supported by fixture precision and conditioning
   before implementation. Any revision needs written evidence, not a failing implementation.
8. Do not copy an OPM experimental model wholesale. Pin the exact equation/lifecycle, preserve
   required source notices for adapted code, and verify it with independent invariants as well
   as OPM output. A translated algorithm and its translation-derived expected values are one
   oracle, not two.

## Dependency and deliverable map

```text
C0 -> C1 -> C2 -> C3 -> C4 -> C5 -> C6
             \-----------------------> C7 (layout work after C0)
FIM-COMPOSITIONAL-SEAM-READY + C6 + C7 -> C8 -> C9 -> C10 -> C11 -> C12 -> C13 -> C14
C14 -> C15 (immiscible water extension)
```

The diagram permits independent work; it does not ask the executor to spawn agents. C1–C6
do not require FIM convergence improvements. C8 and later require the repair milestone.

| Task | Result | Suggested issue/commit boundary |
| --- | --- | --- |
| C0 | Frozen scope, fluid/reference dataset and acceptance contract | Design + fixture provenance |
| C1 | Validated fluid specification and unit conversion | Input model |
| C2 | PR mixture EOS and single-phase properties | EOS kernel |
| C3 | Stable phase classification and scalar PT flash | Stability / flash in separate tested commits |
| C4 | Equilibrium/property derivatives | Derivative contract |
| C5 | Transport properties and surface flash | Viscosity / surface conditions |
| C6 | Standalone thermodynamic readiness gate | Fixture suite + validation doc |
| C7 | Component-aware layout and state | Layout / immutable geometry boundary |
| C8 | Conservative cell storage and residual scaling | Accumulation equations |
| C9 | Conservative face flux and assembly | Flux / gravity separate commits |
| C10 | Linear solve, Newton, retry/commit lifecycle | Linear adapter / update policy |
| C11 | Compositional wells and controls | BHP first, rate limits second |
| C12 | Native external trajectory and refinement validation | Case fixtures + measured acceptance |
| C13 | WASM, worker, input/output and catalog integration | API, worker, UI separate commits |
| C14 | Browser performance, reproducibility and release readiness | Final gate and documentation |
| C15 | Immiscible water extension | Separate V1b implementation issues |

## Fixed design contracts for V1

### State and equations

Let N be the number of hydrocarbon components. Each cell has N independent primaries:
`[p_bar, z_0, ..., z_(N-2)]`; recover `z_(N-1) = 1 - sum(z_0..z_(N-2))`.
`z` is overall **mole** fraction. Cell equations are N component mole balances. Pressure is
not an extra equation appended to those balances. Phase fraction, phase composition and density
are derived by flash at the fixed temperature; phase labels/cached guesses are auxiliary data.

Use `beta` for vapor mole fraction; `x_i`/`y_i` for liquid/vapor mole fractions; `cL`/`cV` for
phase molar densities in mol/m³. They imply:

```text
v_mixture = (1-beta)/cL + beta/cV          [m³/mol]
S_vapor = (beta/cV) / v_mixture
S_liquid = ((1-beta)/cL) / v_mixture
n_i(cell) = PV(cell,p) * (S_liquid*cL*x_i + S_vapor*cV*y_i)  [mol]
R_i = n_i(new) - n_i(previous) + dt*sum(outward face flux_i) - dt*source_i
```

`source_i` is positive for injection, negative for production [mol/day]. Flash phase fraction
is not saturation. Use the derived phase volumes above; do not set `S_vapor = beta`.
In a single-phase state evaluate only its present-phase term; do not divide by an undefined
absent-phase density even when its multiplier would nominally be zero.
No separate pressure closure should overdetermine this system. Compressibility enters through
EOS density and the existing sourced rock pore-volume relation.

### Units and derivative conversion

| Boundary | Contract |
| --- | --- |
| Geometry/transport | Existing m, mD, bar, cP, day and `DARCY_METRIC_FACTOR` conventions |
| Thermodynamics kernel | SI: Pa, K, mol, m³; molecular weight kg/mol; gas constant in J/(mol K) |
| EOS input adapter | `p_Pa = 1e5*p_bar`; absolute temperature, never Celsius in the kernel |
| EOS derivatives back to transport | `df/dp_bar = 1e5 * df/dp_Pa` |
| Viscosity adapter | `mu_cP = 1e3*mu_Pa_s` |
| Phase gravity density | `rho_mass = c_molar * sum(x_i*MW_i)` in kg/m³, not molar density |
| Inventory and sources | mol and mol/day throughout the new residual and conservation ledger |
| Surface output | Flash-produced phase volumes at explicit `p_surface`, `T_surface`; label conditions |

The EOS SI kernel must not call `ReservoirSimulator` or import browser types. Conversion occurs
at named boundaries and has tests. Existing black-oil standard-volume outputs retain their
old meaning; compositional output cannot masquerade as those fields without a documented adapter.

### Proposed module boundary

Create under `src/lib/ressim/src/` only as stages consume the files:

```text
fluid/mod.rs, specification.rs, units.rs
fluid/eos.rs, stability.rs, flash.rs, derivatives.rs, transport.rs
compositional/mod.rs, state.rs, layout.rs, properties.rs
compositional/assembly.rs, flux.rs, wells.rs, newton.rs, timestep.rs, reporting.rs
```

Names may be consolidated for small cohesive code, but preserve ownership: `fluid/` has no
global simulation state; `compositional/` owns new physics/state; `fim/linear/` and the repaired
lifecycle interfaces supply reusable infrastructure. Register modules in `lib.rs` only when
they compile. Do not rename/move the whole existing FIM tree at the start.

Minimum interfaces, expressed as contracts rather than copy/paste signatures:

- `FluidSpecification`: ordered component IDs/properties, symmetric interaction matrix,
  EOS variant, viscosity model, fixed temperature, surface conditions, supported domain/version.
- `flash(p_pa, T_k, z, initial_guess) -> Result<FlashState, FlashError>`: stable phase state,
  beta, compositions, densities, residual diagnostics and deterministic cache hints.
- `evaluate_with_derivatives(...) -> Result<FluidEvaluation, FluidError>`: properties and
  derivatives with respect to pressure and independent z coordinates, phase status and validity.
- `CompositionalState`: accepted primaries, derived-cache ownership and versioned checkpoint.
- `EquationLayout`: cells/components/well blocks, index accessors, pressure columns and row
  metadata. Do not infer a row's physical meaning solely from `index % 3`.
- `AssemblyResult`: unscaled residual/Jacobian, declared row/variable scaling, layout and
  component inventory/source diagnostics. Accepted-state evaluation uses this same physics.

Use explicit enum variants for single-liquid, single-vapor, two-phase and typed failures.
Exact critical degeneracy, unsupported state and numerical nonconvergence are distinct outcomes.
Absent-phase composition/property derivatives must not be read as valid transport contributions.

## C0 — Pin the fluid, consuming case and independent oracle

**Read:** the installed OPM headers (correction 2) —
`/usr/include/opm/material/constraintsolvers/PTFlash.hpp`,
`/usr/include/opm/material/eos/{CubicEOS,PRParams,CubicEOSParams}.hpp`,
`/usr/include/opm/material/fluidsystems/{ThreeComponentFluidSystem.hh,GenericOilGasWaterFluidSystem.hpp}`,
`/usr/include/opm/material/viscositymodels/LBC.hpp`,
`/usr/include/opm/material/components/{C1,C10,SimpleCO2}.hpp`,
`/usr/include/opm/models/ptflash/*.hh`. The compositional wells under
`opm-simulators/flowexperimental/comp/` are **not packaged** and cannot be read here
(correction 3).

1. Record package versions (`dpkg -l | grep opm`), the `dpkg -S` owner of every header cited,
   and the exact source symbols. There are no git checkouts here; do not present the assessment's
   three upstream hashes as reproducible local artifacts, and do not assume the packaged headers
   are the source that built `/usr/bin/flow`.
2. Pin a full fluid dataset with units, temperature, critical data, acentric factors, molecular
   weights, critical volumes, interaction matrix, PR variant/constants, viscosity parameters and
   surface conditions. Correction 4 selects CO2/C1/C10 from `ThreeComponentFluidSystem.hh`; copy
   the numbers out of `components/{SimpleCO2,C1,C10}.hpp` with line references rather than from
   memory. Its all-zero interaction matrix is an explicit choice made by that source
   (`interactionCoefficient()` returns `0.0`), which is what makes it citable — zero BIPs are
   never a universal default.
3. Two oracle capabilities, tracked separately (correction 5).
   **Thermodynamic — available.** Build the header-only OPM PTFlash/CubicEOS/LBC harness with
   `g++ -std=c++20 -I/usr/include -I/usr/include/dune`; no OPM build, link step or MPI is needed.
   Commit its source, exact command and outputs.
   **Trajectory — blocked.** `libopm-simulators-bin` ships only `/usr/bin/flow`, which is the
   black-oil simulator at 2026.04. Do not fabricate a `flow --comp` flag. Mark external
   trajectory parity BLOCKED and leave it blocked until a compositional executable is actually
   built; the harness does not substitute for it, and C12 cannot close without it.
4. Store proposed fixtures under `opm/compositional/` with a README, generator/replay command,
   source/license provenance and small machine-readable outputs. Large raw runs belong outside
   Git. Include binary/build identity, fluid/deck checksum, units, solver tolerances and phase
   state. A hand-transcribed chart is not a high-precision flash oracle.
5. Define first flow fixture: a 1D uniform column, no gravity/capillarity, fixed-temperature
   hydrocarbon mixture, one injection and one production boundary/well, with composition chosen
   to exercise a phase change inside the declared domain. C12 replaces temporary boundary
   sources with the admitted well model and validates matched physical controls.
6. Create `docs/COMPOSITIONAL_VALIDATION.md` (proposed) containing the matrix below, approved
   tolerances, model exclusions and exact reference commands. Do not claim gates pass yet.

**Exit:** the dataset and numerical acceptance contract are reproducible and complete. Source
properties and a version number alone do not satisfy the reference-executable requirement.

## C1 — Fluid specification and unit-safe input

**Create:** `fluid/specification.rs`, `units.rs`, small public/internal API in `mod.rs`.

1. Parse a typed specification; validate finite positive critical pressure/temperature and
   molecular weight, valid component count/order, symmetric finite N×N interaction matrix,
   permitted EOS/viscosity variant, and positive absolute temperatures/surface pressure.
   Validate model-specific parameter ranges from C0 rather than inventing universal ranges.
2. Validate z length, finite nonnegative entries, and sum-to-one tolerance. Tiny roundoff
   correction may be explicitly specified; reject materially invalid compositions. Preserve
   true zero components using an active-component policy rather than silently adding material.
3. Unit-test bar/Pa, Pa·s/cP, mass/mole conversion and derivative chain factors. Component ordering
   must survive serialize/deserialize roundtrip. Changing order without reordering interactions
   must be rejected or handled by an explicit permutation operation.
4. Add typed errors that identify field/component and reason. No `unwrap()` on user input.

**Tests to create:** `comp_spec_*`, `comp_units_*`. **Exit:** deterministic validation and
roundtrip tests pass; no changes to black-oil simulator creation.

## C2 — PR mixture EOS and single-phase properties

**Create:** `fluid/eos.rs`. Keep scalar implementation first.

1. Write and source PR pure-component coefficients, temperature correction, mixture a/b and
   binary-interaction convention. Pin the PR versus modified-PR choice; OPM has distinct
   parameter branches and rounded constants need not produce identical results.
2. Solve the compressibility-factor cubic robustly, retaining admissible real roots. Check
   `Z > B`, logarithm domains, finite values, and near-repeated roots. Largest/smallest root
   labels alone do not establish thermodynamic stability; root candidates feed C3.
3. Evaluate log fugacity coefficients, molar density and mass density. Use numerically stable
   expressions near small B and the ideal-gas limit. An invalid logarithm is an error, not a clamp.
4. Test against C0 scalar fixtures for pure-component, mixture, low-pressure and multiple-root
   states. Add independent ideal-gas and dimensional limits. Include at least one state where
   two algebraic roots must not be treated as two coexisting stable phases.

**Tests:** `comp_eos_*`. **Exit:** properties and root residuals meet frozen tolerances.
No flash, global Newton or viscosity fitting in this task.

## C3 — Stability and scalar PT flash

**Create:** `fluid/stability.rs`, `flash.rs`; use
`/usr/include/opm/material/constraintsolvers/PTFlash.hpp` as the sourced algorithm reference.

1. Implement stable-phase testing with a specified tangent-plane/stability criterion and
   multiple appropriate trial seeds. A Wilson K estimate is only an initial guess. Do not
   conclude single phase merely because a Rachford–Rice root is absent for one guessed K vector.
2. For a two-phase state, solve the **vapor-fraction** Rachford–Rice equation
   `sum(z_i*(K_i-1)/(1+beta*(K_i-1))) = 0`, bounded to the physical interval. OPM does **not**:
   `PTFlash.hpp` solves `rachfordRice_g_` for `L`, the **liquid** mole fraction, so every
   comparison against harness output needs `beta = 1 - L` applied explicitly at the boundary.
   Never interchange the symbols.
3. Compute `x_i=z_i/(1+beta*(K_i-1))`, `y_i=K_i*x_i`; update K from fugacity equilibrium with
   bounded iterations and sourced acceleration/fallback. Validate fugacity residual, both
   phase normalizations and reconstructed z before returning success.
4. For a stable single phase, return the correct density/composition and exact physical
   phase amount. Specify any incipient-phase estimates as auxiliary, not conserved material.
5. Handle zero/trace components with the C1 active-set policy. Test warm versus cold starts;
   the converged physical state must not depend on stale guesses. Return explicit numerical
   failure near unsupported degeneracy rather than relabeling an unstable mixture as single phase.
6. Write separate commits/tests for stability and two-phase flash. The latter may use a
   deliberately fixed-K helper test, but fixed K is not the delivered equilibrium model.

**Tests:** `comp_stability_*`, `comp_flash_*`; binary and ternary, liquid-only, vapor-only,
two-phase, bubble/dew crossings, trace/zero components, poor initial guesses, iteration exhaustion.
**Exit:** phase-state, conservation, equilibrium and failure-contract matrix passes.

## C4 — Differentiate converged thermodynamics

**Create:** `fluid/derivatives.rs`; reuse `fim/ad.rs` arithmetic only if the dependency remains
clean, or mechanically move that small generic utility through a separately validated commit.

1. Define equilibrium unknown vector y and equations `F(y,u)=0`, with
   `u=[p_Pa,z_0..z_(N-2)]` at fixed temperature. Pin independent equations/normalizations so
   the local Jacobian is square and nonsingular away from phase/critical boundaries.
2. Compute `dy/du = -F_y^{-1} F_u` through a linear solve, not an explicit matrix inverse.
   Carry derivatives into compositions, beta, density, saturation and fugacity-dependent
   properties. OPM scalar solve followed by derivative reconstruction is the reference pattern.
3. In stable single phase, derive properties on that branch without solving a degenerate
   two-phase derivative system. For exact switches specify the one-sided/active-phase policy;
   near-critical ill-conditioning must produce a diagnostic or supported-domain rejection.
4. Verify derivative of dependent `z_last` is -1 for each independent composition coordinate.
   Finite-difference perturbations must preserve overall normalization and stay on the intended
   smooth branch. Compare warm/cold flash derivatives and cached/uncached results.
5. Run FD step sweeps, tightening scalar flash tolerance so flash noise is below the derivative
   signal. Print the variable, phase state, step, analytic/FD values and condition diagnostics
   on mismatch. Do not demand a central derivative across a discontinuous phase boundary.

**Tests:** `comp_derivatives_*`. **Exit:** source equations, linear solve residual and FD/AD
comparisons pass for pressure and every independent composition coordinate; fail explicitly on
unsupported singular derivative states. Do not send a partially filled derivative array to FIM.

## C5 — Viscosity, transport properties and surface conditions

**Create:** `fluid/transport.rs`, surface-flash API and tests.

1. Implement the exact viscosity correlation pinned in C0, including all required parameters
   and derivatives. OPM's generic compositional fluid system includes an LBC path; do not assume
   a viscosity correlation follows automatically from PR. Constant viscosity is allowed only
   in a separately labeled diagnostic fixture, not as an undeclared product approximation.
2. Convert phase molar density to mass density for gravity. Verify saturation reconstruction
   from phase mole amounts and molar volumes; test beta != vapor saturation cases.
3. Flash a produced overall component stream at the declared surface pressure/temperature.
   Compute surface liquid/gas volumes from phase mole rates and molar densities. Keep total
   component moles unchanged. Define zero-flow and single-phase outputs without dividing by zero.
4. Document the single-stage surface assumption. Do not reuse `Bo`, `Bg`, or `Rs` as an
   arbitrary-composition separator calculation. Do not label all liquids as stock-tank oil
   unless the reporting contract explicitly defines that surface liquid quantity.

**Tests:** `comp_transport_*`, `comp_surface_*`. **Exit:** positive finite in-domain properties,
correct derivatives and component-conservative surface conversion agree with reference fixtures.

## C6 — Standalone thermodynamic admission

1. Run all `comp_` tests implemented so far and verify matched counts. Rebuild the fixture
   comparison using the pinned external reference, not expected values generated by Rust itself.
2. Sweep the C0 pressure/composition envelope deterministically with explicit bubble/dew and
   near-critical samples. Record physical successes, typed unsupported states, numerical
   failures, iterations and derivative error. A sweep with arbitrary failed cells excluded is
   not a successful domain validation.
3. Mark the near-critical domain supported only if its equilibrium/derivative gates pass;
   otherwise narrow the declared domain and reject those inputs. Distinguish numerical
   unsupported-domain limits from claims about the underlying fluid physics.
4. Commit small fixtures, tests and `COMPOSITIONAL_VALIDATION.md` results. If standalone Rust
   modules cannot be compiled for WASM because of a dependency, resolve that before integration.

**Exit:** THERMO-READY. External flash parity blocked by missing executable remains explicitly
blocked; green internal invariants alone do not satisfy this milestone.

## C7 — Component layout, independent state and geometry boundary

**Create:** `compositional/state.rs`, `layout.rs`; consume F7's reviewed interfaces.

1. Make component/cell/well offsets explicit. Use N=2 and N=3 production layouts; include a
   synthetic N=4 layout-only test to expose hidden three-component assumptions. Keep all
   existing black-oil offsets unchanged.
2. Reuse geometry/rock arrays through a small read-only view or owned shared data structure.
   Do not give EOS functions a whole `ReservoirSimulator` merely to access pressure or porosity.
3. Define accepted state versus Newton trial state, flash-cache validity key, previous inventory,
   and a versioned checkpoint. Caches are performance hints and cannot determine physical truth.
4. Decide AD seeding explicitly. For bounded component counts, dispatch to typed cell/face
   kernels with N and 2N seeds; avoid relying on unsupported const-generic expressions.
   Use explicit supported instantiations or a reviewed dynamic derivative design if required.
   A dummy third equation is not an acceptable two-component model.
5. Map pressure columns and component equations into the shared linear metadata without
   classifying them as water/oil/gas. New reporting identifies components by stable IDs.

**Tests:** `comp_layout_*`, `comp_state_*`; bijective indexing, tail offsets, serialization,
component permutation, clone independence and cache invalidation. **Exit:** state and matrix
layout support both component counts with no changes to black-oil results.

## C8 — Cell inventory, accumulation and scaling

**Prerequisites:** THERMO-READY, C7, FIM-COMPOSITIONAL-SEAM-READY.

1. Implement the stated molar inventory equation and existing pressure-dependent rock PV with
   named units. Evaluate previous accepted inventory once from the previous accepted state;
   it is constant with respect to current Newton primaries.
2. Assemble N component residuals and N×N local Jacobian. Compare values against a simple
   independent inventory calculation and derivatives against FD in both phase regimes.
3. Define dimensionless row and primary-variable scaling from physical reference quantities,
   with a declared floor for absent/trace components. Store scaling metadata in diagnostics.
   Do not divide by an exactly zero component inventory or reuse black-oil surface-volume CNV
   scaling blindly. Prevent a large component from hiding an unconverged trace component.
4. Establish one-cell constant-state zero residual, imposed-source inventory accounting,
   pressure/composition perturbation and component-permutation tests. Flux-free closed-cell
   equilibrium should remain stationary; an artificial depletion source is accounted explicitly.

**Tests:** `comp_accumulation_*`, `comp_scaling_*`. **Exit:** independent inventory and Jacobian
checks pass. The new model has exactly as many independent equations as primaries.

## C9 — Component face flux and global assembly

**Create:** `compositional/flux.rs`, `assembly.rs`.

1. Reuse transmissibility geometry and its metric conversion. At zero capillarity/gravity,
   compute phase Darcy reservoir-volume rate with phase relative permeability/viscosity;
   multiply by upstream molar density and phase composition for each component molar flux.
   Pin the hydrocarbon relperm law/table in C0/C12; existing water/oil curves are not implicitly
   a hydrocarbon liquid/vapor law.
2. Determine upstream side by each phase potential, then freeze that branch for one Jacobian
   evaluation. Differentiate all dependencies on both neighboring cells. Test away from zero
   potential with FD and test the exact zero-potential branch convention separately.
3. Insert equal and opposite flux contributions into the two cells. Test two-cell exchange,
   global cancellation of internal faces, uniform-state stationarity, both flow directions,
   and phase appearance across a face. Add a 1D column with boundary source accounting.
4. Assemble the full sparse Jacobian and compare every entry on tiny grids against an
   independent numerical Jacobian, with a fixed previous state and no hidden cache mutation.
5. In a separate commit, add gravity using mass density and existing bar/m/day conventions.
   Validate a sourced single-phase hydrostatic equilibrium before a multiphase gravity case.
   Keep hydrocarbon capillary pressure zero in V1; unequal phase pressures require a separately
   derived equilibrium contract and cannot be enabled by passing an old capillary flag.

**Tests:** `comp_flux_*`, `comp_assembly_*`, `comp_gravity_*`. **Exit:** conservative scalar
residual, derivative agreement and hydrostatic tests pass; global composition is conserved
without wells. No Newton damping changes in this task.

## C10 — Integrate linear solve, Newton and timestep lifecycle

1. Start with direct solve on small nonsingular systems for a correction-quality oracle.
   Use F4's full-system diagnostics. A direct-solver success flag alone is insufficient.
2. Add the component-aware block-ILU/iterative adapter and compare full residuals/corrections.
   Audit every hard-coded triple in scaling, block extraction and pressure mapping. Establish
   a correct small-grid baseline before adding compositional CPR.
3. If CPR is needed for C14 performance, create a separate subtask specifying pressure
   restriction weights from the new accumulation/volume derivatives, the well contribution
   policy, units and coarse operator. Test against direct corrections and refined trajectories.
   Do not copy black-oil quasi-IMPES weights because matrix sizes happen to match.
4. Use the repaired global lifecycle with model-specific trial update: p remains in the
   thermodynamic domain; z stays on the simplex. Compute a fraction-to-boundary step bound
   including dependent z_last, then evaluate trial residuals and flash validity. Do not
   independently clamp/renormalize z after taking a step, which changes the Newton direction.
5. The policy for an exactly zero component must allow physically supplied material to enter
   without permanently freezing that coordinate. Use the C1/C3 active-set contract and explicit
   zero-component injection tests. Do not require every component to exceed an arbitrary floor.
6. Accept only a trial whose component residuals, well residuals and admissibility checks meet
   the frozen contract. Failure reduces dt/retries within a fixed budget; no time, inventory,
   well cumulative or history update occurs before commit. Roll back flash caches as specified.
7. Test one-cell depletion, 1D displacement, phase appearance/disappearance, forced local-flash
   failure, forced linear failure, rejected timestep/retry and restart. Diagnostics must
   distinguish flash, linear, nonlinear, admissibility and budget failures.

**Tests:** `comp_linear_*`, `comp_newton_*`, `comp_rollback_*`. **Exit:** supported tiny cases
finish conservatively with finite corrections; failed runs return actionable diagnostics and
unchanged accepted state. No changes to black-oil damping/default tolerances.

## C11 — Compositional wells and control switching

**Read:** repaired black-oil topology/Peaceman code. OPM's `CompWellModel` and
`CompWellFlash.hpp` are **not available in this environment** (correction 3): either fetch
upstream `opm-simulators` source under the normal approval rules, or derive the well contract
from first principles plus the installed `opm/models/ptflash/` residual headers and say which
was done. Do not paraphrase a header that was not read.

1. Write a well design note before coding: unknowns/equations, signed reservoir/molar rates,
   injection composition, mixture mobility/density, BHP datum, connection source and surface
   conversion. Reuse geometry and physical-well IDs, not black-oil fluid splitting.
2. Start with one completion and BHP control. Derive reservoir/component connection sources
   and their reservoir/BHP derivatives using the same fluid evaluation as reservoir assembly.
   For injection use the prescribed composition and a sourced injectivity/mobility rule; merely
   using kr of an absent injected phase can incorrectly eliminate injectivity.
3. Specify whether wellbore flash/mixing is required by the selected formulation. If modeled
   well unknowns require it, solve/evaluate it coherently; do not port only a source conversion
   while omitting the well state equations that define that source.
4. Add specified total **molar** rate control with BHP limits, then admitted surface volumetric
   control through C5 conversion. Write units in the API and tests. A surface volumetric target
   is not reservoir q and not a black-oil RESV target.
5. Validate component-source sum, local/global residual and derivative equality, and
   Schur-recovered correction against an explicit full solve. The selected well source must
   be the same source counted in field material balance and reporting.
6. Add multiple completions sharing one BHP in a separate commit; account for depth and mixture
   head when gravity is enabled. Crossflow must be implemented and tested or explicitly
   rejected as outside V1, never silently clipped. Test control switching with both feasible
   and infeasible rate targets.

**Tests:** `comp_well_bhp_*`, `comp_well_rate_*`, `comp_well_schur_*`, `comp_well_multiperf_*`.
**Exit:** supported well controls have physical source, AD/FD and inventory oracles; exclusions
are enforced before simulation. #27/#28 fixes are inherited through verified geometry/interfaces,
not assumed to prove new compositional well equations correct.

## C12 — Validate native trajectories against independent references

1. Freeze matched fixtures for: static closed mixture; phase-changing single-cell depletion;
   1D composition displacement; one BHP producer/injector; rate/BHP switching. Run binary and
   ternary cases where meaningful. Preserve source/deck/property versions and well units.
2. Compare field pressure, phase saturations, component inventories, injection/production
   mole totals, phase appearance times and surface volumes. Compare per-cell states at selected
   report times; matching one aggregate curve can hide spatial or component errors.
3. Run at least three timestep resolutions, then spatial refinement separately. Normalize
   errors with predeclared absolute floors for near-zero components; compare a refined external
   solution, not merely a coarse Flow output. No requirement for identical Newton counts.
4. A published compositional benchmark such as SPE5 is an optional later expansion only when
   its full component count, fluid/well physics and data are supported. Do not truncate a
   published mixture to three components and retain the original benchmark name/oracle.
5. Record model domain, errors, runtime, iterations, retries and exact commands in the owning
   validation document. External state/units mismatches are INCONCLUSIVE, not candidate failures.

**Tests:** bounded `comp_reference_*`; longer cases explicitly ignored with a dedicated release
runner. **Exit:** NATIVE-COMPOSITIONAL-READY, with frozen independent acceptance bands and all
supported cases passing. Browser exposure waits for this milestone.

## C13 — WASM, worker, output and catalog integration

**Existing files:** `frontend.rs`, `lib.rs`, `src/lib/simulator-types.ts`,
`src/lib/buildCreatePayload.ts`, `src/lib/workers/sim.worker.ts`,
`src/lib/stores/runtimeStore.svelte.ts`, `src/lib/charts/runQuantities.ts`, catalog scenarios.

1. Add a discriminated fluid-model payload with separate black-oil and compositional config.
   Reject unsupported component counts, phases, controls and old flags before allocating a run.
   Preserve old serialized config meaning; do not interpret an absent discriminator as compositional.
2. Expose typed creation, stepping, diagnostics and snapshot access. Rebuild WASM and verify
   native/WASM results on the same tiny fixtures. No native OPM dependency ships to the browser.
3. Serialize component IDs/units, p/z, derived phase state and conservation diagnostics.
   Avoid per-step cloning of unnecessary large per-cell derivative arrays. Worker messages
   remain plain structured-cloneable data; use the existing runtime snapshot/post boundary.
4. Add versioned checkpoint roundtrip and incompatible-schema rejection. If no public restart
   UI exists, retain the native roundtrip contract and expose restart only through a tested API.
5. Add component inventory/production and phase-composition quantities through the chart/quantity
   registry; spatial z/saturation views belong to visualization. Use existing CurveConfig panels.
   Do not silently feed compositional output into black-oil GOR/material-balance analytical curves.
6. Admit one scenario with explicit FIM/compositional capability, references, supported controls,
   fixed-temperature/phase exclusions and meaningful sensitivity. Keep unsupported cases out
   of the picker. Display actionable flash/domain failures without implementation jargon.

**Validation:** WASM build, worker/API roundtrip tests, native/WASM parity, `pnpm run validate`
while iterating and `pnpm run validate:full` before committing; new compositional gates also run
explicitly until added to the authoritative validation scripts. Follow the add-scenario skill.
**Exit:** the browser runs the validated case and existing black-oil scenarios remain unchanged.

## C14 — Performance, repeatability and final admission

1. Benchmark fixed cases at 1 cell, a 1D column and increasing 2D/3D grids on named native and
   browser environments. Record cells/components/steps, EOS calls/iterations, assembly/linear
   time, retries, peak memory and snapshot payload size. Warm caches and cold starts separately.
2. Before optimization, freeze a performance budget suited to the intended browser case. If
   it fails, identify the dominant measured cost; optimize cache reuse or linear algebra in
   separate commits while preserving equilibrium and correction-quality gates. Do not relax
   accuracy or suppress phase-stability checks without a separately proven safe criterion.
3. Confirm deterministic results within declared floating-point tolerances across restart,
   repeat run and native/WASM paths. Verify cancellation/error handling leaves a usable UI.
4. Run full existing-product gates and the complete compositional admission matrix on final
   committed source and rebuilt WASM. Record known unsupported states explicitly.
5. Update validation docs, user-facing scope, skills/commands where needed and issue links.
   Commit the final evidence. Do not publish unless requested. Call V1 complete only when the
   claimed feature's entire validation matrix passes; otherwise keep it experimental/hidden.

## C15 — V1b: immiscible water, a separate extension

1. Specify immiscible water as an additional conserved component with its own density/FVF,
   viscosity and optional water–hydrocarbon capillary relation. No hydrocarbon dissolution in
   water and no water vaporization under this scoped model.
2. Use N hydrocarbon primaries `[p,z_0..z_(N-2)]` plus `Sw`: N+1 unknowns and equations
   (N hydrocarbon balances plus water). Hydrocarbon phase saturations sum to `1-Sw`; flash
   beta remains a mole split within hydrocarbons. Handle the zero-hydrocarbon limit explicitly.
3. Extend accumulation, flux, gravity, well injection/control and reporting together. Pressure
   and phase-pressure conventions must be written before capillarity is enabled. Retain zero
   hydrocarbon capillary pressure unless a coherent unequal-pressure EOS contract is added.
4. Test zero-water reduction to V1, water-only/small-hydrocarbon limits, closed inventory,
   three-phase closure, water injection into hydrocarbon-filled cells and hydrocarbon injection
   into water-filled cells. Near pure water, z may become unidentifiable; implement a sourced
   primary adaptation/regular formulation or reject that domain rather than inventing equations.
5. Repeat the C8–C14 gates for the enlarged layout. This is not a one-flag activation of the
   current black-oil `three_phase_mode`, and it does not deliver CO2/brine thermodynamics.

## Numerical acceptance matrix to freeze in C0

The following targets are starting specifications for well-conditioned fixtures, not claims
about the current code. C0 must record exact absolute scales and any conditioning exclusions.

| Quantity | Proposed admission target and oracle |
| --- | --- |
| Composition normalization | Absolute error <=1e-12; no materially negative component |
| Flash z reconstruction | Max absolute component error <=1e-10 |
| Two-phase equilibrium | Max absolute log-fugacity ratio <=1e-8 for active components |
| Scalar EOS/flash fixture | Relative 1e-7 for density/fugacity, absolute 1e-8 for beta/composition; fixtures must have enough precision to resolve these errors. If the fixture is coarser, obtain a better oracle before claiming this target |
| Smooth property derivatives | <=1e-4 relative error using a frozen nonzero absolute derivative scale, demonstrated over an FD step plateau |
| Tiny assembled Jacobian | <=1e-5 scaled entrywise/column error versus FD at smooth states; separate branch-boundary tests |
| Tiny direct linear oracle | Full-system relative residual <=1e-10 for declared well-conditioned fixtures; finite correction and row partitions mandatory |
| Local conservative face exchange | Opposite contributions cancel within roundoff using the same face flux |
| Global closed/source balance | <=1e-8 relative accumulated mole error per component on admission fixtures, with explicit absolute mole floor and independently integrated sources |
| Refined external trajectories | Set per observable in C0/C12 from oracle accuracy and refinement; target <=1% for cumulative quantities on smooth simple fixtures, explicit absolute/state/event tolerances required |
| Newton acceptance | Component/well scaled tolerances derived and frozen in C8/C11 to satisfy inventory/trajectory targets; no generic borrowed black-oil numeric tolerance |

Critical/switching states may lack a classical two-sided derivative; that changes the test
oracle, not permission to waive equilibrium or conservation. A performance budget never overrides
these accuracy contracts.

## Commands and gate implementation

Existing commands, from repository root:

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml comp_ -- --nocapture
cargo fmt --manifest-path src/lib/ressim/Cargo.toml -- --check
bash scripts/build-wasm.sh
pnpm run validate:full
cargo test --manifest-path src/lib/ressim/Cargo.toml benchmark_buckley -- --nocapture
git diff --check
```

The first command is **future-facing**: it is valid only after C1 creates tests; use
`cargo test --manifest-path src/lib/ressim/Cargo.toml -- --list` and source inspection to verify
the intended filter actually executes tests. Narrow to each task's prefix during development.
By C6 create `scripts/validate-compositional.sh` (proposed) with `thermo`, `native`, and eventually
`wasm` modes, explicit filter inventories, nonzero executed-test checks and bounded runtimes.
Do not claim this script exists until implemented. Keep long reference replays separate from
fast gates. Add gate commands to package/CI/skills only after the underlying tests exist.

For the C6 compile-only WASM check, after ensuring the target is installed, use:

```bash
cargo check --manifest-path src/lib/ressim/Cargo.toml --target wasm32-unknown-unknown
```

This checks compilation only; it does not replace execution-based native/WASM parity in C13.

Shared Rust changes also need the FIM repair plan's locked tests and WASM control matrix.
For new fluid-only modules, run focused tests and existing required Rust/product pre-commit
gates; do not run an expensive OPM trajectory after every scalar helper edit.
Use `uv run --directory tools/opm_flow ...` for Python fixture tooling, adapting only to an
actually implemented CLI. Never fabricate parser support for compositional summary fields.

## Completion evidence and stop conditions

For every task, write to its issue and the owning validation/design document:

```text
C-task / start and final commit / supported component and phase count:
Source equation, OPM path+hash, fluid/deck checksum and unit convention:
Changed interfaces and held-constant behavior:
Tests created, exact commands, executed counts, failures and artifact locations:
Worst equilibrium / derivative / conservation / trajectory error, with state and scale:
Native/WASM coverage; missing external oracle or unsupported domain:
Completed gate, remaining blocker, next permitted C-task:
```

Stop dependent work if component counts/equations disagree, a flash failure becomes a valid
state silently, FD comparisons cross an unclassified switch, surface and reservoir units are
mixed, rejected steps mutate inventory, or the external oracle's model/state cannot be matched.
Report the smallest failing fixture and missing decision. Do not call a thermodynamics library
a completed compositional simulator, or a passing native case a completed browser feature.

## Copyable executor prompt

```text
Execute the next uncompleted task in docs/COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md,
starting with C0 if no committed handoff exists. Read #29, the task's child issue and the
repository skills. Stay within the declared V1 scope. Check the FIM readiness milestone
before any C8-or-later integration. Planned module/test/script names do not exist until
their owning task creates them; verify all executed test filters match tests.
Pin the fluid data, units, source equations and independent acceptance oracle before coding.
Do not invent property data, OPM flags, missing derivatives or a success result after flash
failure. At a blocked numerical decision, produce the smallest fixture and precise evidence
needed for review; continue only independent work. Commit validated changes, update the
owning issue and record the required handoff. Do not push or expose unvalidated physics.
```
