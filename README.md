# ResSim

Browser-based reservoir simulator with a Rust/WASM flow engine, Svelte 5 UI, analytical reference overlays, and 3D visualization. The current product is strongest as a teaching and diagnostic tool for waterflood, sweep, depletion, gas injection, and early black-oil studies.

## Current State

- 14 canonical scenarios across waterflood, sweep, depletion, gas, and black-oil benchmark domains, including three "decision-insight" cases demonstrating history-match non-uniqueness, parameter-interaction amplification, and PVT representation risk.
- Two-phase oil/water IMPES workflow validated against Buckley-Leverett breakthrough references.
- Analytical overlays for Buckley-Leverett, Craig areal sweep, Dykstra-Parsons vertical sweep, Stiles-style combined sweep, Dietz pseudo-steady-state depletion, Fetkovich decline, Arps decline, and Havlena-Odeh material-balance diagnostics.
- Black-oil PVT mode is available for volatile-oil style studies through correlation-based or tabular PVT input.
- Three-phase oil/water/gas flow is implemented, but still treated as experimental because comparative-solution validation is not complete.
- Public app runs are IMPES-first while the FIM path is deferred to explicit developer diagnostics.
- OPM Flow reference work is handled offline through precomputed artifacts; browser execution remains local WASM.

## Scenario Inventory

| Domain | Key | Primary Analytical Reference | Notes |
|--------|-----|------------------------------|-------|
| Waterflood | `wf_bl1d` | Buckley-Leverett + Welge | 1D immiscible displacement baseline |
| Waterflood | `wf_tornado` | None (simulation-only) | kv x density-contrast interaction — individually-small parameters combine into a dominant driver |
| Sweep | `sweep_areal` | Craig confined five-spot | Quarter-pattern style interpretation |
| Sweep | `sweep_vertical` | Dykstra-Parsons | Non-communicating layered sweep baseline |
| Sweep | `sweep_combined` | Stiles or Dykstra-Parsons combined with BL | Scenario-owned analytical method toggle |
| Depletion | `dep_pss` | Dietz pseudo-steady-state | Shape factor varies with producer location |
| Depletion | `dep_decline` | Fetkovich exponential decline | Constant-PVT decline reference |
| Depletion | `dep_arps` | Arps decline + material balance diagnostics | Layered / volatile-oil style depletion study |
| Depletion | `dep_nct` | Dietz PSS decline (shared reference) | N·c_t material-balance ambiguity — matched history, 4x different recovery factor |
| Gas | `gas_injection` | Gas-oil Buckley-Leverett | Three-phase gas injection with analytical breakthrough |
| Gas | `gas_drive` | Simulation-first with p/z and MB diagnostics | Qualitative gas-drive study pending stronger validation |
| Gas | `dep_pvt` | None (simulation-only) | Two black-oil PVT tables, one calibration point — undersaturated compressibility representation risk (three-phase, grouped under Gas) |
| Benchmark | `spe1_gas_injection` | Published Eclipse results (Odeh, 1981) | SPE1 black-oil benchmark with per-layer dz, PVT table, single-layer wells |

## Implemented Capabilities

### Flow Physics

- IMPES pressure-saturation splitting on a 3D Cartesian grid with per-layer cell thickness support.
- Two-phase oil/water flow with Corey relative permeability.
- Optional Brooks-Corey oil-water and oil-gas capillary pressure.
- Optional gravity with density-weighted hydrostatic head.
- Three-phase oil/water/gas transport with Stone II oil relative permeability, gas Corey curves, explicit gas transport, and gas-phase CFL handling.
- Correlation-based or tabular black-oil PVT support with bubble-point tracking, Rs liberation/re-dissolution, pressure-dependent mobility, and producing GOR reporting.
- Peaceman-style well model with BHP or rate control, per-layer completion, dynamic PI updates, and injector / producer switching logic. Well PI uses per-layer cell thickness.
- Per-layer initial conditions: water saturation, gas saturation, and cell thickness can be specified per z-layer for scenarios with gas caps or non-uniform geology.
- Adaptive timestep checks based on saturation change, pressure change, and well-rate change limits.

### Analytical and Diagnostic Surfaces

- Buckley-Leverett fractional-flow reference curves with Welge shock construction.
- Craig areal sweep, Dykstra-Parsons vertical sweep, and Stiles-style combined sweep interpretation.
- Dietz depletion, Fetkovich exponential decline, and Arps decline overlays.
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

## Model Validity Notes

- Buckley-Leverett is a 1D immiscible displacement reference. Do not interpret it as a general areal or heterogeneous-field predictor.
- Craig areal sweep applies to confined five-spot style pattern assumptions. It is context, not a universal areal flood model.
- Dykstra-Parsons assumes layered, non-communicating flow. When the simulator allows vertical communication, analytical sweep penalties are intentionally conservative.
- Stiles-style combined sweep improves layered recovery interpretation, but it is still an analytical teaching aid rather than a substitute for full streamline or field-scale pattern modeling.
- Three-phase mode remains experimental because validation depth still trails implementation breadth.
- Material-balance diagnostics are partial by phase: water and gas cumulative closure are reported explicitly; oil remains the residual phase in current diagnostics.
- The Brooks-Corey capillary model is numerically capped at `20 x P_entry`. That cap is a stability safeguard, not a physical plateau.
- Pore volume is held constant within each timestep. Rock compressibility enters the pressure equation accumulation term but does not update cell geometry. This is the standard IMPES simplification and is consistent with the compressibility magnitudes used.
- Water density and viscosity are pressure-independent. This is adequate for the reservoir pressure and temperature ranges targeted by this simulator.
- Two-phase PVT mode uses the scalar undersaturated oil compressibility `c_o` for the accumulation term. In three-phase mode, an effective oil compressibility is computed from the bubble-point curve and blended with the undersaturated value over a 5-bar margin near the bubble point, keeping the accumulation term continuous across phase-state transitions.
- Numerical derivatives of PVT properties (effective gas compressibility, saturated Bo/Bg slopes used in three-phase accumulation) use a fixed 1-bar finite-difference step. Accuracy degrades below roughly 5 bar, which the pressure floor prevents from being reached in practice.

## Why The Roadmap Is Ordered This Way

The next priorities follow standard reservoir-engineering practice:

- Comparative-solution benchmarking should precede more physics expansion for black-oil and three-phase work.
- Analytical methods should only be exposed where their assumptions remain explicit and defensible.
- Relative permeability, PVT, and sweep-method interpretation dominate uncertainty more than UI breadth does.

That ordering aligns with the literature already used in the project: Buckley and Leverett, Welge, Craig, Dykstra and Parsons, Stiles, Dietz, Fetkovich, Arps, Havlena and Odeh, and the SPE comparative-solution tradition used for simulator validation.

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
| `docs/THREE_PHASE_IMPLEMENTATION_NOTES.md` | Three-phase implementation details and remaining validation gaps |
| `docs/UNIT_SYSTEM.md` | Unit conventions, equations, and PVT / solver notes |
| `docs/DOCUMENTATION_INDEX.md` | Which documents are authoritative vs historical |

## Near-Term Focus

See `ROADMAP.md` for the full ordering. The next major engineering priorities are:

1. Black-oil and three-phase validation.
2. Remaining scenario / benchmark architecture consolidation.
3. Output-selection and comparison-model cleanup.
4. Multi-case inspection and data export.
5. Gas-cap and extended pattern physics only after the validation backlog is closed.
