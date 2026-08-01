# ResSim

A reservoir simulator that runs entirely in the browser and draws every result against the
classical solution it is supposed to reproduce. Three-phase black-oil flow on a 3D Cartesian
grid — implicit-pressure (IMPES) or fully implicit — with Peaceman wells, gravity, capillarity
and correlation or tabular PVT, plotted live against Buckley–Leverett, Craig, Dykstra–Parsons,
Stiles, Dietz, Fetkovich, Arps, Havlena–Odeh and line-source well-test solutions, plus
precomputed OPM Flow runs where no closed form exists. Fifteen scenarios ship as
self-contained studies, each with sensitivity dimensions, a 3D view, saturation profiles, and
an explicit statement of what its reference assumes and where the simulation is expected to
leave it. Nothing is uploaded: the solver is WebAssembly running on the page.

## What You Can Do With It

- **Watch a numerical solution meet — or leave — its analytical reference.** Every waterflood,
  sweep, depletion and gas-injection case carries the reference curve beside the simulation, on
  a shared pore-volume or time axis.
- **Ask why they differ.** Cases are built so that the departure has one cause at a time:
  grid resolution, timestep, solver formulation, capillarity, gravity, layering, or mobility
  ratio. Several ship a control run that lands back on the analytical curve, which is what makes
  the rest of the departure attributable.
- **Sweep a sensitivity.** Each scenario owns its dimensions — mobility ratio, Corey exponents,
  residual oil, gravity number, density contrast, vertical communication, drainage geometry,
  skin, PVT representation — and runs every selected variant as its own curve.
- **Check the simulator against another simulator.** Selected cases bundle a precomputed OPM
  Flow run of the identical deck, so the comparison is not only against theory.
- **See the field, not just the chart.** A 3D view of pressure, water/gas saturation,
  permeability and porosity, plus a saturation-versus-distance profile with the analytical
  front drawn on it, both stepping through the same replay timeline as the charts.
- **Read the caveats the product raises itself.** Enabled gravity, capillarity, a
  non-one-dimensional grid or an off-centre well each downgrade the analytical status and say
  so, rather than leaving a reference curve looking authoritative where it does not apply.

## How To Use It

1. **Pick a scenario** from the picker at the top. The groups say what kind of question each
   answers: 1D displacement, sweep efficiency, flow regimes and decline, simulation-only, or
   validation benchmarks. The description below the picker states the case, its solver, and the
   reference solution it is judged against.
2. **Choose a sensitivity dimension**, then toggle the variants to include. The dimension
   description explains what the study is for; each variant chip carries what it changes.
3. **Run.** Progress and any solver warnings appear beside the controls; the run stays
   responsive because the solver executes in a Web Worker.
4. **Read the comparison.** Simulation curves are solid, analytical references dashed, and any
   external reference (published data or OPM Flow) dotted. Panels can be expanded individually,
   and the x-axis switches between time, pore volumes injected and cumulative injection.
5. **Inspect the field.** The 3D view and the profile plot below it replay the same timeline.
   For a flooded case the profile shows the simulated saturation against the Buckley–Leverett
   front at that moment.
6. **Change parameters** in the sections below the charts to depart from the shipped case; the
   analytical status updates to tell you when a reference has stopped being applicable.

## Scenario Inventory

| Catalog group | Scenario | Key | Primary reference / purpose |
|---|---|---|---|
| 1D Displacement — Buckley–Leverett | 1D Waterflood | `wf_bl1d` | Buckley–Leverett + Welge analytical reference; also hosts the FIM-vs-IMPES formulation comparison |
| 1D Displacement — Buckley–Leverett | 1D Waterflood — Capillary Effects | `wf_capillary` | Departure from the zero-capillary BL limit; physical versus numerical front spreading |
| 1D Displacement — Buckley–Leverett | Gravity-Stable vs Unstable Displacement | `wf_gravity_stability` | 1D vertical column flooded upward or downward; gravity along the flow path brackets the viscous BL curve instead of bounding it |
| 1D Displacement — Buckley–Leverett | Numerical Dispersion & Convergence | `wf_numerics` | The case where no BL assumption is broken, so the whole gap is the grid: first-order convergence over a 40x cell-size range, the IMPES stability limit made visible, IMPES vs FIM, and OPM Flow runs at two resolutions |
| 1D Displacement — Buckley–Leverett | Gas Injection | `gas_injection` | Gas-oil fractional-flow breakthrough |
| Sweep Efficiency | Areal Sweep | `sweep_areal` | Craig confined five-spot correlation |
| Sweep Efficiency | Vertical Sweep | `sweep_vertical` | Dykstra–Parsons / Stiles layered sweep |
| Sweep Efficiency | Layer Crossflow — Do the Layers Talk? | `sweep_crossflow` | Dykstra–Parsons' non-communicating assumption tested directly: a k_v/k_h ladder the correlation cannot see, a crossflow benefit that reverses sign with mobility ratio, and capillary crossflow that needs a path |
| Sweep Efficiency | Combined Sweep | `sweep_combined` | Combined areal and vertical contact with selectable layered correlation |
| Sweep Efficiency | Gravity Override (Dietz Tongue) | `wf_gravity` | Vertical sweep lost to a gravity tongue; gravity-off control returns to BL, rate/density/k_z/completion ladders leave it; OPM Flow cross-check of the base case |
| Flow Regimes & Decline | Transient Radial Flow (Theis) | `dep_welltest` | Line-source drawdown before boundaries are felt; permeability, skin, and near-well grid bias |
| Flow Regimes & Decline | Drainage Geometry & Productivity (Dietz) | `dep_pss` | Equal-area drainage geometries and well positions; C_A recovered from the measured PSS drawdown |
| Flow Regimes & Decline | Boundary-Dominated Decline (Fetkovich) | `dep_decline` | Finite-slab transient, boundary arrival, and asymptotic decline |
| Flow Regimes & Decline | Layered Depletion (Arps) | `dep_arps` | Spatial layered depletion approaching a late-time Dietz/Fetkovich superposition, plus a crossflow limitation study |
| Flow Regimes & Decline | Gas Reserves from p/z | `dep_gas_pz` | Dry-gas depletion against the p/z material-balance straight line; pore compressibility, compartmentalisation and how much history you have each inflate the reserves estimate |
| Simulation Only — No Analytical Reference | Solution Gas Drive | `gas_drive` | Black-oil FIM sensitivities; optional OPM Flow benchmark |
| Simulation Only — No Analytical Reference | PVT Model Risk — One Calibration Point | `dep_pvt` | Two PVT representations constrained at one point |
| Validation Benchmarks | SPE1 Black-Oil Benchmark | `spe1_gas_injection` | Published Eclipse and OPM Flow comparative-solution references |

## Reading The Results — Model Validity Notes

What each reference is entitled to claim, and where it stops.

- Buckley-Leverett is a 1D immiscible displacement reference. Do not interpret it as a general areal or heterogeneous-field predictor.
- Craig areal sweep applies to confined five-spot style pattern assumptions. It is context, not a universal areal flood model.
- Dykstra-Parsons assumes layered, non-communicating flow. When the simulator allows vertical communication, analytical sweep penalties are intentionally conservative.
- Stiles-style combined sweep improves layered recovery interpretation, but it is still an analytical teaching aid rather than a substitute for full streamline or field-scale pattern modeling.
- Three-phase mode is graded quantitatively against numerical references (OPM Flow, SPE1), not promoted tank-model overlays. A Tarner–Tracy model was evaluated for Solution Gas Drive but rejected because its uniform tank assumptions do not represent the case's localized BHP drawdown and initially mobile free gas. Vaporized oil (Rv) is not modelled, so wet-gas and gas-condensate behavior is outside the envelope. See `docs/THREE_PHASE_VALIDATION.md` section 6.
- Material-balance closure is reported explicitly for all three phases. Oil is the residual *saturation* in transport (S_o = 1 - S_w - S_g), but its diagnostic is direct: reported surface oil production versus actual stock-tank oil inventory depletion.
- The Brooks-Corey capillary model is numerically capped at `20 x P_entry`. That cap is a stability safeguard, not a physical plateau.
- Pore volume is held constant within each timestep. Rock compressibility enters the pressure equation accumulation term but does not update cell geometry. This is the standard IMPES simplification and is consistent with the compressibility magnitudes used.
- Water density and viscosity are pressure-independent. This is adequate for the reservoir pressure and temperature ranges targeted by this simulator.
- Two-phase PVT mode uses the scalar undersaturated oil compressibility `c_o` for the accumulation term. In three-phase mode, an effective oil compressibility is computed from the bubble-point curve and blended with the undersaturated value over a 5-bar margin near the bubble point, keeping the accumulation term continuous across phase-state transitions.
- Numerical derivatives of PVT properties (effective gas compressibility, saturated Bo/Bg slopes used in three-phase accumulation) use a fixed 1-bar finite-difference step. Accuracy degrades below roughly 5 bar, which the pressure floor prevents from being reached in practice.

## Quick Start

### Prerequisites

- Node.js 18+ with `pnpm`
- Rust toolchain
- `wasm-pack`
- `wasm32-unknown-unknown` target

### Install

```bash
pnpm install
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

### Run

```bash
pnpm run dev
```

### Validate

```bash
pnpm run validate           # frontend: typecheck + lint + test + build
pnpm run validate:product   # + Rust IMPES solver bucket
bash scripts/validate-solver-coverage.sh all   # Rust solver test buckets
```

Note: full `cargo test` is not used as a gate — FIM diagnostic tests can dominate runtime (see `docs/FIM_DEFERRED_BACKLOG.md` and `.claude/skills/ressim-validation/SKILL.md`).

## Implemented Capabilities

### Status

- 15 canonical scenarios grouped as 1D displacement, sweep efficiency, flow regimes and decline, simulation-only, and validation benchmarks.
- Two-phase oil/water IMPES workflow validated against Buckley-Leverett breakthrough references.
- Analytical overlays for Buckley-Leverett, Craig areal sweep, Dykstra-Parsons vertical sweep, Stiles-style combined sweep, Dietz pseudo-steady-state depletion, Fetkovich decline, Arps decline, line-source well-test drawdown, and Havlena-Odeh material-balance diagnostics.
- Black-oil PVT mode is available for volatile-oil style studies through correlation-based or tabular PVT input.
- Three-phase oil/water/gas flow is validated against comparative solutions: SPE1 Case 1 for gas injection and an OPM Flow reference for solution gas drive (`docs/THREE_PHASE_VALIDATION.md`).
- Two-phase cases use scenario-declared IMPES defaults, while black-oil cases and the dedicated solver-formulation exhibit expose FIM in the public app.
- OPM Flow reference work is handled offline through precomputed artifacts; browser execution remains local WASM.

### Flow Physics

- IMPES pressure-saturation splitting on a 3D Cartesian grid with per-layer cell thickness support.
- Two-phase oil/water flow with Corey relative permeability.
- Optional Brooks-Corey oil-water and oil-gas capillary pressure.
- Optional gravity with density-weighted hydrostatic head.
- Three-phase oil/water/gas transport with Stone II oil relative permeability, gas Corey curves, explicit gas transport, and gas-phase CFL handling.
- Correlation-based or tabular black-oil PVT support with bubble-point tracking, Rs liberation/re-dissolution, pressure-dependent mobility, and producing GOR reporting.
- Peaceman-style well model with BHP or rate control, per-layer completion, dynamic PI updates, and injector / producer switching logic. Well PI uses per-layer cell thickness.
- Eclipse-style wellbore datum: a well's BHP is quoted at a datum depth (default the shallowest completion) and carried down to each completion by a wellbore column whose density is derived from the completion fluids, or fixed per well. Active only when gravity is enabled.
- Per-layer initial conditions: water saturation, gas saturation, and cell thickness can be specified per z-layer for scenarios with gas caps or non-uniform geology.
- Adaptive timestep checks based on saturation change, pressure change, and well-rate change limits.

### Analytical and Diagnostic Surfaces

- Buckley-Leverett fractional-flow reference curves with Welge shock construction, in time, pore-volume and spatial-profile form.
- Craig areal sweep, Dykstra-Parsons vertical sweep, and Stiles-style combined sweep interpretation.
- Dietz depletion, Fetkovich exponential decline, and Arps decline overlays.
- Line-source (exponential-integral) well-test drawdown with semilog slope fitting for permeability and skin.
- Havlena-Odeh material-balance terms and drive indices in depletion diagnostics.
- p/z-style gas diagnostics and producing GOR outputs for gas-oriented cases.
- Comparison metrics such as MAE, RMSE, and MAPE for selected overlays.

### UI and Workflow

- Scenario-first input workflow through `ScenarioPicker.svelte`.
- Scenario-owned sensitivity dimensions with per-variant run sweeps.
- Worker-based execution to keep the UI responsive.
- 3D scalar visualization for pressure, water saturation, gas saturation, permeability, and porosity.
- Shared chart layout system for runtime and comparison views.
- Scenario-first case selection with scenario-owned parameters, sensitivities, and references.

## Validation Status

### Verified

- Rust benchmark cases compare 1D waterflood breakthrough timing against Buckley-Leverett reference behavior.
- Frontend and catalog tests cover scenario contracts, analytical overlay wiring, chart layout behavior, and payload generation.
- Analytical-contract tests verify that scenario dimensions marked `affectsAnalytical: true` actually perturb the analytical result.

### In Progress

- SPE1 black-oil benchmark scenario is defined with published Eclipse reference overlays and offline OPM Flow artifact hooks. Quantitative match remains a validation task.

### Still Needed

- Quantitative SPE1 acceptance criteria once tabular SCAL and surface-rate control are implemented.
- Stronger three-phase acceptance tests before gas cases can be described as production-grade.
- Additional chart-model coverage for preview-only and per-variant depletion comparison flows.

## Why The Roadmap Is Ordered This Way

The next priorities follow standard reservoir-engineering practice:

- Comparative-solution benchmarking should precede more physics expansion for black-oil and three-phase work.
- Analytical methods should only be exposed where their assumptions remain explicit and defensible.
- Relative permeability, PVT, and sweep-method interpretation dominate uncertainty more than UI breadth does.

That ordering aligns with the literature already used in the project: Buckley and Leverett, Welge, Craig, Dykstra and Parsons, Stiles, Dietz, Fetkovich, Arps, Havlena and Odeh, and the SPE comparative-solution tradition used for simulator validation.

## Project Layout

```text
src/
  App.svelte
  app.css
  main.ts
  lib/
    analytical/
    catalog/
    charts/
    physics/
    ressim/
    stores/
    ui/
    visualization/
    workers/
docs/                 # authoritative + active working docs (see DOCUMENTATION_INDEX.md)
  ARCHITECTURE_NOTES.md
  BENCHMARK_MODE_GUIDE.md
  DOCUMENTATION_INDEX.md
  FIM_STATUS.md
  SOLVER_COMPARISON_SUMMARY.md
  P4_TWO_PHASE_BENCHMARKS.md
  THREE_PHASE_IMPLEMENTATION_NOTES.md
  UNIT_SYSTEM.md
  ...
.archive/             # superseded experiments, closed plans, historical snapshots
  docs/               # (git-tracked, reversible; see .archive/README.md)
ROADMAP.md
TODO.md
```

## Documentation Map

| Document | Purpose |
|----------|---------|
| `ROADMAP.md` | Future-facing roadmap and prioritization |
| `TODO.md` | Active execution tracker |
| `docs/ARCHITECTURE_NOTES.md` | Current architecture direction and unresolved design decisions |
| `.archive/docs/DELIVERED_WORK_2026_Q1.md` | Archived delivered work moved out of TODO |
| `docs/BENCHMARK_MODE_GUIDE.md` | Benchmark workflow semantics and chart behavior |
| `docs/P4_TWO_PHASE_BENCHMARKS.md` | Buckley-Leverett benchmark methodology and tolerance policy |
| `docs/BLACK_OIL_VALIDATION.md` | SPE1 acceptance criteria, black-oil grid convergence, solver safeguards |
| `docs/THREE_PHASE_VALIDATION.md` | Three-phase exit criteria, OPM Flow / SPE1 acceptance, phase-closure diagnostics |
| `docs/THREE_PHASE_IMPLEMENTATION_NOTES.md` | Three-phase implementation details and parameter reference |
| `docs/UNIT_SYSTEM.md` | Unit conventions, equations, and PVT / solver notes |
| `docs/DOCUMENTATION_INDEX.md` | Which documents are authoritative vs historical |

## Near-Term Focus

See `ROADMAP.md` for the full ordering. The next major engineering priorities are:

1. Black-oil and three-phase validation.
2. Remaining scenario / benchmark architecture consolidation.
3. Output-selection and comparison-model cleanup.
4. Multi-case inspection and data export.
5. Gas-cap and extended pattern physics only after the validation backlog is closed.
