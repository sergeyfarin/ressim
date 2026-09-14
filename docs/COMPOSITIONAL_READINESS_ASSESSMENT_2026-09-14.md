# Compositional readiness assessment

Date: 2026-09-14. Reviewed ResSim commit
`ffaf18f30f6d1cea5c7b097a26dc10d4d496303e` after `git pull --no-rebase`
reported “Already up to date.” This is an architecture/source audit with focused native
tests, not a new convergence experiment or a compositional benchmark.

## Recommendation

Detailed follow-through: [FIM repair execution plan](FIM_REPAIR_EXECUTION_PLAN_2026-09-14.md)
and [compositional fluid execution plan](COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md).
These are implementation instructions; their tasks are not recorded as completed by this audit.

Compositional simulation is feasible, but it is a substantial new fluid-model implementation,
not an extension of the current PVT table. Reuse the grid, finite-volume geometry, AD arithmetic,
sparse linear algebra, well topology, and browser execution infrastructure. Introduce a separate
compositional state/residual path rather than adding more meanings to the existing black-oil
`hydrocarbon_var`.

First repair the demonstrated well/Jacobian contract failures and establish trustworthy
component-conservation and phase-transition gates. Develop an independently tested EOS/flash
module alongside that work. Do not wait for universal Flow iteration-count parity, and do not
start by tuning damping, raising Newton limits, or implementing AMG. Successful Newton convergence
and time-discretization accuracy are different acceptance criteria.

Here “mechanics” means nonlinear state updates, phase changes, wells, and linearization. Coupled
solid geomechanics is not a prerequisite for isothermal compositional Darcy flow; it would be a
separate model expansion.

## What the current engine actually implements

Paths below are relative to `src/lib/ressim/src/` unless stated otherwise.

| Area | Source evidence | Reuse and required change |
| --- | --- | --- |
| Cell state | `fim/state.rs`: `FimCellState { pressure_bar, sw, hydrocarbon_var, regime }`; `HydrocarbonState` is saturated/undersaturated | Three primaries, with the last switching between Sg and Rs. No arbitrary component composition. Replace for the new model. |
| Thermodynamics | `pvt.rs`: PVTO/PVDG-style tables; `fim/flash.rs::resolve_cell_flash` resolves Rs/Sg and bubble point | Useful black-oil reference path, not an EOS equilibrium flash. Add critical properties, acentric factors, molecular weights, binary interactions, temperature, phase compositions, density and viscosity models. |
| Conservation | `fim/properties.rs::component_inventory_generic`: `[PV Sw/Bw, PV So/Bo, PV Sg/Bg + oil_sc Rs]` | These ARE component balances, but for three black-oil surface-volume components. General compositional transport needs a declared mass or molar basis and composition-weighted phase inventories/fluxes. |
| Fluxes and derivatives | `fim/flux.rs`: three component fluxes and `Ad<6>` for two neighboring three-primary cells; `fim/assembly_ad.rs`: 3x3 blocks | Reuse Darcy geometry/upwinding patterns and `Ad<const N>` arithmetic. Generalize cell/face seed widths, assembly indexing, property derivatives, and component flux loops. |
| Linear solve | `fim/linear/mod.rs`, `gmres_block_jacobi.rs`, `well_schur.rs` | Sparse solvers, Schur elimination and block-size metadata are reusable; parts of block ILU already use a runtime block size. CPR restriction, pressure extraction, capture schemas and row partitions still require a component-aware audit. This is not a total linear-solver rewrite. |
| Newton and acceptance | `fim/newton.rs`, `newton/damping.rs`, `newton/convergence.rs`, `fim/scaling.rs` | Scaling explicitly indexes water/oil/gas triples. Reuse solve/retry orchestration only behind model-specific update, admissibility, scaling and convergence contracts. Sw/Rs chops do not define valid mole-fraction updates. |
| Wells | `fim/wells*.rs`, `fim/wells_inner.rs`, `fim/flow_resv.rs` | Reuse Peaceman geometry and topology. Injection composition, produced component rates, mixture-dependent well properties, surface flash and control equations are new semantics. The scoped gas-RESV route is not a generic compositional well model. |
| Reporting and API | `reporting.rs`, `frontend.rs`, `lib.rs`, `src/lib/simulator-types.ts`, worker payloads | Current rates/GOR/FVF/state fields assume black oil. Add component metadata and inventories, phase compositions and explicit surface conditions. Preserve existing scenario contracts through a distinct model selection. |

The AD infrastructure is a genuine advantage, but differentiating the current black-oil residual
correctly cannot supply missing compositional physics. Similarly, a successful Rs-to-Sg switch
does not validate hydrocarbon liquid/vapor stability near a critical point.

For an isothermal model with N total conserved components, a pressure/overall-composition
formulation can use pressure plus N−1 independent overall mole fractions. A formulation with
separately treated immiscible water has a different layout; specify that choice before indexing
unknowns. At fixed temperature, representative component storage is
`V phi sum_phases(S_phase * molar_density_phase * mole_fraction_phase,component)`.
Advective component flux uses the same phase compositions/densities with Darcy phase rates.
Equilibrium requires equality of component fugacities between coexisting hydrocarbon phases,
composition normalization, and a phase-stability decision. These closures must be differentiated
consistently with the global residual.

## OPM available on this machine

`/usr/bin/flow --version` returned **`flow 2026.04`**. The sandbox blocks MPI socket initialization;
the version check succeeded outside it. No Flow simulation or new OPM build was performed in this
audit. `/usr/bin/flow*` contains only `flow`; a compositional executable has not been established
as available. An installed black-oil Flow executable is not proof that a compositional deck can
be run with it.

Clean local source checkouts inspected:

| Checkout | Commit |
| --- | --- |
| `OPM/opm-simulators` | `062cb19986aa8f11cffc30351fd2fee355d0ccb4` |
| `OPM/opm-common` | `04ee6aa5ba6a4f461a202627b3f9c7ea9f0f9eae` |
| `OPM/opm-models` | `6a73e7cac0631d4f4f4a68ae5fccb578415a615f` |

These hashes identify source references, not a claim that they built the installed binary or
that the three revisions form a tested build combination. In particular, the compositional
Flow headers use models under `opm-simulators/opm/models`; the separate `opm-models` checkout
is an additional architectural reference, not an assertion about the executable's include tree.

Useful implementation references:

- `OPM/opm-common/opm/material/eos/CubicEOS.hpp` and related PR/SRK parameter classes:
  cubic EOS and mixture parameters.
- `OPM/opm-common/opm/material/constraintsolvers/PTFlash.hpp`: stability testing,
  Rachford–Rice, composition solves, then `updateDerivatives_`. Its `solve` first obtains
  equilibrium in scalar arithmetic and then reconstructs derivatives. This is a valuable
  pattern for Rust: implicit differentiation of the converged local equilibrium, with tests,
  rather than blindly differentiating every iteration and branch of a flash solver.
- `OPM/opm-models/opm/models/ptflash/flashindices.hh`, `flashprimaryvariables.hh`,
  `flashlocalresidual.hh`: pressure/overall-composition layout and component-wise accumulation
  and transport. This local residual uses mass fractions and mass density; do not combine it
  with molar formulas without molecular-weight conversion.
- `OPM/opm-simulators/flowexperimental/comp/flow_comp.cpp` and `flow_comp.hpp`:
  dispatch by component count/water option, a FlashModel, equation-sized matrix blocks, and a
  dedicated `CompWellModel`.
- `OPM/opm-simulators/flowexperimental/comp/wells/CompWellFlash.hpp`: scalar/AD wellbore
  flash and component masses. This illustrates why reservoir flash alone is insufficient.

OPM's [Flow overview](https://opm-project.org/?page_id=19) describes its black-oil foundation;
the [2026 release/news page](https://opm-project.org/?page_id=287) also links current
compositional development presentations. Neither substitutes for testing the exact executable,
fluid specification and case used as an oracle.

Best reuse strategy: port selected algorithms/contracts into the Rust model and use a pinned OPM
compositional build offline for reference fixtures. Linking OPM C++ into the browser is a separate
build/runtime project involving its dependency stack; invoking native OPM behind a server is
another deployment architecture. Neither is a small change to this Rust/WASM simulator.

## Serious findings and existing issue ownership

1. **Well derivative and stencil contracts need adjudication before reuse.**
   [#27](https://github.com/sergeyfarin/ressim/issues/27) concerns the gas-injector
   surface-pressure derivative versus finite differences;
   [#28](https://github.com/sergeyfarin/ressim/issues/28) concerns a one-cell local well stencil
   versus a nine-cell control stencil. These are test/implementation inconsistencies, not yet
   proof that a particular production formula is wrong. Check the intended source equation,
   FD perturbation/stencil, live AD assembly, and Schur-recovered correction before fixing
   either implementation or oracle. Compositional coupling would make unresolved derivatives
   harder to diagnose.
2. **The curated FIM gate omits these module-local contracts.**
   `scripts/validate-solver-coverage.sh` runs `fim::tests::wells::`, whereas these tests are in
   `fim::wells::tests::`. The names look similar but select different suites. This is a concrete
   local coverage gap in addition to [#13](https://github.com/sergeyfarin/ressim/issues/13)'s CI
   gap: `.github/workflows/pr-tests.yml` runs the Rust IMPES bucket, not the shared/FIM buckets.
   Repair or explicitly adjudicate known failures, then include the focused derivative/stencil
   contracts in a gate that actually executes them. Do not conceal them by weakening assertions.
3. **No general EOS/component/flash contract exists in the current Rust model.** This is a
   capability gap, not a regression. It spans fluid input, equilibrium and derivatives,
   conservation, well sources, initialization/restart, API and reference fixtures. A thermodynamic
   prototype is not ready for product coupling until all these boundaries have owners and gates.
   Readiness work is recorded in [#29](https://github.com/sergeyfarin/ressim/issues/29).
4. **Existing black-oil validation debt remains relevant but case-specific.** Issues
   [#10](https://github.com/sergeyfarin/ressim/issues/10),
   [#11](https://github.com/sergeyfarin/ressim/issues/11),
   [#12](https://github.com/sergeyfarin/ressim/issues/12), and
   [#25](https://github.com/sergeyfarin/ressim/issues/25) track gravity wells, depletion disagreement,
   reference coverage, and gas inventory/reporting respectively. They were read as open tracker
   items, not reproduced here. Resolve those affecting the first compositional case; do not
   infer a universal FIM failure from their titles.
5. **Some guidance is materially stale.** The engine skill calls FIM dev-only and says oil
   balance lacks an explicit diagnostic. Current code defaults `fim_opm_aligned_nonlinear` to
   true, product documentation describes shipped FIM scenarios, and `reporting.rs` records
   `material_balance_error_oil_m3` against inventory depletion. That diagnostic is useful but
   not arbitrary-component conservation certification. Likewise, the execution-plan header
   stops at WATER-011 although later registry/worklog entries record promoted changes.
   Follow the latest applicable evidence and code; reconcile these summaries under
   [#24](https://github.com/sergeyfarin/ressim/issues/24) before using them as an implementation
   checklist.

## Does FIM convergence have to be solved first?

The historical registry records promoted OpmAligned raw-state retention (`FIM-STATE-001`),
default flavor selection (`FIM-FLAVOR-001`), tabulated Corey evaluation (`FIM-RELPERM-001`), and
singular-direct fallback routing (`FIM-LINEAR-014`). These supersede an assessment based only on
the earlier gas plateau or blanket claims that stronger linear solvers are the next lever.
Their historical timings/counts were not rerun here and are not new baselines.

Also read [the worklog](FIM_CONVERGENCE_WORKLOG.md), WATER-028: on its matched water case,
timestep refinement reduced the recorded FOPT difference from 7.96% to 0.05%. It corrects
WATER-027's attribution to a shared reservoir-physics defect. This is case-specific historical
evidence, not proof of general oil accuracy or resolution of every item in
[#21](https://github.com/sergeyfarin/ressim/issues/21).

The prerequisite is **a trustworthy solver/model boundary**, with:

- validated reservoir and well Jacobians, including phase boundaries and control switches;
- consistent stored state, equilibrium state, property endpoint extension, accumulation and
  accepted-state updates; rejected-step rollback also includes flash caches/phase state;
- independent component inventory/source closure and timestep-refined output checks;
- comparable full-system linear residuals, finite corrections and reservoir/well partitions;
- explicit supported-case completion and runtime limits in native and WASM execution.

A missing backend-neutral diagnostic makes a comparison **INCONCLUSIVE**, not REFUTED. A
partial EOS or phase-switch port cannot refute the complete OPM lifecycle it omits. Matching
Flow's Newton iteration count is not an accuracy gate, and accepted raw Newton intermediates
must not be confused with admissible final physical states.

## Proposed implementation sequence and effort

Estimates below are engineering judgments, not measured schedules. They assume one experienced
reservoir-numerics developer, existing geometry/UI reuse, and a deliberately limited isothermal
hydrocarbon model. Unknown derivative defects and critical-region behavior can expand them.

| Stage | Deliverable and exit gate | Indicative effort |
| --- | --- | --- |
| 0. Foundation | Adjudicate #27/#28; gate these contracts; define first case, units, component basis and independent reference executable | 2–6 weeks, uncertainty high |
| 1. Thermodynamics, independently of stage 0 | Small PR EOS fluid package; stable single/two-phase flash, derivatives and explicit failure outcomes. Validate published/pinned OPM fixtures, fugacity equality, normalization, conservation, dilute limits and near-critical behavior | 4–8 weeks |
| 2. Native compositional flow | Separate state/residual path; N-component storage/flux, generalized layout/scaling/CPR wiring. Closed-cell conservation, 1D displacement, phase appearance/disappearance, FD Jacobians and timestep/grid refinement | 6–12 weeks after prerequisites |
| 3. Wells and browser product | Specified-composition injection, component production, surface flash, controls, initialization/checkpoint roundtrip, worker/UI metadata, bounded browser performance and OPM trajectories | 6–12 weeks |

A restricted research demonstrator is roughly a **3–6 month** project; a validated browser feature
roughly **6–12+ months** including integration and iteration. General industrial compositional
capability is a much larger undertaking. These are overlapping planning ranges, not additive
fixed-price estimates. Component count increases dense cell-block storage roughly quadratically
and local factorization cost roughly cubically, in addition to per-cell flash work; profile early
before committing to large browser cases.

Start with two or three hydrocarbon components, fixed temperature, and one or two hydrocarbon
phases. Declare whether immiscible water is included. Initially exclude thermal transport,
reactive chemistry, aqueous CO2 dissolution/brine, diffusion and geomechanics. If the actual goal
is CO2/brine storage, define that separately: it needs an appropriate aqueous thermodynamic model,
not just a hydrocarbon PR flash. Start with closed cells and a simple displacement case, then
consider a compositionally specified SPE5-style case; SPE1 black oil is not a compositional oracle.

## Audit validation

Focused native results on the reviewed committed Rust tree:

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml fim::tests::wells:: -- --nocapture
# test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 524 filtered out
cargo test --manifest-path src/lib/ressim/Cargo.toml fim::wells::tests:: -- --nocapture
# test result: FAILED. 16 passed; 2 failed; 0 ignored; 0 measured; 511 filtered out
cargo test --manifest-path src/lib/ressim/Cargo.toml assembly_ad -- --nocapture
# test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 516 filtered out
```

The two failures are exactly the tests named in #27/#28. The derivative assertion is
`(target_exact - target_fd).abs() / target_scale < 1e-3`; the stencil assertion compares
`[4]` with `[0, 1, 2, 3, 4, 5, 6, 7, 8]`. This reproduces those open inconsistencies on
`ffaf18f30f6d1cea5c7b097a26dc10d4d496303e`; it does not adjudicate their root causes.
The 13 passing AD tests establish only their local consistency contracts, not OPM parity.
Temporary logs: `/tmp/ressim-compositional-wells.log`,
`/tmp/ressim-compositional-well-contracts.log`, `/tmp/ressim-compositional-ad.log`.

Full `cargo test`, the full product
gate and the WASM convergence matrix are deliberately not used as documentation-change gates.
No solver code, tolerances, scenarios, generated artifacts or OPM sources were changed.
