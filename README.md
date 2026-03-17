# ResSim — Browser-Based 3D Reservoir Simulator

A two- and three-phase IMPES reservoir simulator built with **Rust/WASM** (physics core) and **Svelte 5 + Vite** (frontend). It provides interactive 3D visualization, production-rate charting with reference-solution comparisons, and benchmark-based validation — all running entirely in the browser.

## Current State (2026-03)

- **Active simplification refactor**: replacing the 4-layer "case library + editability policies" navigation with a simple `pick scenario → optionally pick sensitivity → run` flow. New model: `src/lib/catalog/scenarios.ts` + `ScenarioPicker.svelte` (see `REFACTOR.md` — steps 4 and 7 pending).
- **Reference-solution overlays**: permissive for approximate cases, with clearly visible caveats.
- **Three-phase support** (experimental): oil/water/gas simulation via Stone II relative permeability; no analytical reference solution.
- **Execution model**: all scenarios initialize and run directly in browser-side WASM; no pre-run artifact pipeline.

## Features

### Simulation Engine (Rust → WASM)

- **IMPES solver** — implicit pressure, explicit saturation on a 3D Cartesian grid.
- **Two-phase oil/water** flow with Corey relative permeability and optional Brooks-Corey capillary pressure.
- **Three-phase oil/water/gas** flow (experimental): Stone II k_ro, Corey k_rg, oil-gas capillary pressure, explicit gas saturation transport, CFL check extended to gas.
- **Corey relative permeability** — configurable endpoints (`S_wc`, `S_or`, `S_gc`, `S_gr`) and exponents (`n_w`, `n_o`, `n_g`).
- **Brooks-Corey capillary pressure** — optional oil-water and oil-gas curves with physical caps.
- **Gravity** — configurable toggle with phase-density-weighted hydrostatic head.
- **Permeability modes**: uniform, random (optional deterministic seed), per-layer CSV input.
- **User-editable porosity**, initial water saturation, and rock compressibility.
- **Well model**: Peaceman PI, rate or BHP control, per-layer completion, auto BHP constraint switching. Dynamic PI update each timestep.
- **Material-balance error** tracking returned per timestep.
- **PCG solver convergence** warning surfaced in the UI.
- **Modular Rust layout**: `lib.rs` (WASM API), `step.rs`, `solver.rs`, `relperm.rs`, `capillary.rs`, `well.rs`, `grid.rs`.

### Reference Solutions and Comparisons

- **Buckley-Leverett** fractional-flow reference solution for waterflood cases.
  - Welge tangent construction, Sw profile, post-breakthrough outlet Sw via bisection.
  - Breakthrough via cumulative PVI tracking.
- **Depletion decline** — PSS reference solution `q(t) = q₀·exp(−t/τ)` with Dietz shape-factor PI.
  - 1D slab and 2D radial drainage geometry support.
  - Per-layer PI summation for multi-layer cases.
- Mismatch metrics: MAE, RMSE, MAPE displayed in the rate chart.

### Visualization & UI

- **3D property view** (Three.js) — selectable properties: pressure, water/oil/gas saturation, permeability (x/y/z), porosity. Interactive legend with Fixed / Percentile range modes.
- **Rate chart** — collapsible Rates, Cumulative, Diagnostics panels with 21 curves. X-axis modes: time, log-time (Fetkovich), PVI, cumulative liquid/injection.
- **Sw Profile chart** — cell-index saturation profile compared to the reference flood front.
- **Scenario picker** (active refactor) — 7 predefined benchmark/reference scenarios plus custom mode. Each scenario is a complete self-contained parameter set with optional sensitivity variants.
- **Sensitivity sweeps** — run one simulation per variant; stored results feed comparison charts.
- **Reference-to-custom handoff** — start from any scenario and switch to custom editing while preserving source provenance.
- **Worker-based stepping** keeps UI responsive. Replay/history controls with time slider.
- **Simulation progress indicator** (step X / N).
- **Dark/Light theme** toggle.

### Validation & Benchmarks

Buckley-Leverett breakthrough PVI benchmarks (Rust unit tests):

| Case | Analytical PV_BT | Simulator PV_BT | Relative Error |
|------|------------------:|----------------:|---------------:|
| BL-Case-A (favorable mobility) | 0.586 | 0.609 | 4.0% |
| BL-Case-B (adverse mobility) | 0.507 | 0.553 | 9.0% |

Refined discretization (nx=96, dt=0.125d) reduces errors to about 2.5–3.1%, confirming that the remaining mismatch is dominated by coarse-grid and coarse-timestep numerical effects.

Current benchmark scenarios (`src/lib/catalog/scenarios.ts`):

| Key | Class | Sensitivity |
|-----|-------|-------------|
| `wf_bl_case_a` | Waterflood | Grid refinement (24/48/96 cells) |
| `wf_bl_case_b` | Waterflood | Grid refinement (24/48/96 cells) |
| `wf_mobility_study` | Waterflood | Oil viscosity (0.5/1.0/5.0 cP) |
| `dep_dietz_center` | Depletion | None |
| `dep_dietz_corner` | Depletion | None |
| `dep_fetkovich` | Depletion | None |

## Unit System

All quantities use **oil-field metric units**:

| Quantity | Unit |
|----------|------|
| Pressure | bar |
| Distance | m |
| Time | day |
| Permeability | mD |
| Viscosity | cP |
| Compressibility | 1/bar |
| Transmissibility factor | `8.527×10⁻³` (DARCY_METRIC_FACTOR) |

## Quick Start

### Prerequisites

- Node.js ≥ 18 + npm
- Rust toolchain (`cargo`, `rustup`)
- `wasm-pack` (`cargo install wasm-pack`)
- WASM target: `rustup target add wasm32-unknown-unknown`

### Install & Run

```bash
npm install
npm run dev        # builds WASM (predev hook), starts Vite dev server
```

### Production Build

```bash
npm run build      # runs build:wasm → vite build
npm run preview    # serve production bundle locally
```

### Tests

```bash
npm test                           # Vitest frontend tests
cd src/lib/ressim && cargo test    # Rust unit + benchmark tests
```

> **Known non-blocking warning**: large JS chunk warning during production build.
> `src/lib/ressim/pkg/simulator.js` is wasm-pack generated — do not convert manually.

## Project Layout

```
src/
├── App.svelte                      — main UI: state, controls, scenario management
├── app.css                         — global styles
├── main.ts                         — app entry point
└── lib/
    ├── analytical/                 — analytical Svelte components + Buckley-Leverett helper logic/tests
    ├── charts/                     — RateChart, ChartSubPanel, SwProfileChart, and chart helpers/tests
    ├── visualization/              — Three.js 3D grid rendering + legend
    ├── workers/                    — Web Worker bridge to the WASM simulator
    ├── simulator-types.ts          — TypeScript interfaces for worker payloads
    ├── buildCreatePayload.ts       — payload builder + tests
    ├── catalog/                    — scenarios.ts (new) + legacy catalog code (being removed, see REFACTOR.md)
    ├── ui/                         — ScenarioPicker, feedback surfaces, controls, cards, and section panels
    ├── components/ui/              — primitive UI controls (Button, Card, Input, Select, Collapsible)
    └── ressim/src/                 — Rust simulator core
        ├── lib.rs                  — WASM API surface
        ├── step.rs                 — IMPES timestep logic (2-phase and 3-phase)
        ├── solver.rs               — PCG pressure solver
        ├── relperm.rs              — Corey rel-perm (2-phase) + Stone II (3-phase)
        ├── capillary.rs            — Brooks-Corey capillary pressure (oil-water + oil-gas)
        ├── well.rs                 — well model + validation
        └── grid.rs                 — grid cell definitions
scripts/
└── build-wasm.sh                   — WASM build script
public/
└── cases/                          — curated preset scenarios
docs/                               — technical reference docs (see below)
```

## Key Documentation

| Document | Content |
|----------|---------|
| `docs/DOCUMENTATION_INDEX.md` | Map of authoritative docs |
| `docs/BENCHMARK_MODE_GUIDE.md` | Benchmark scenario reference guidance and chart defaults |
| `docs/P4_TWO_PHASE_BENCHMARKS.md` | BL benchmark methodology, tolerances, and results |
| `docs/THREE_PHASE_IMPLEMENTATION_NOTES.md` | Three-phase (Stone II) architecture and parameter reference |
| `docs/UNIT_SYSTEM.md` | Comprehensive unit system reference |
| `docs/UNIT_REFERENCE.md` | Quick unit lookup card |
| `docs/TRANSMISSIBILITY_FACTOR.md` | Derivation of `8.527×10⁻³` constant |
| `REFACTOR.md` | Active simplification refactor working document |
| `TODO.md` | Prioritized work items and product roadmap |

## Physics Summary

### Implemented ✅

| Feature | Details |
|---------|---------|
| Two-phase oil/water flow | IMPES pressure-saturation splitting |
| Three-phase oil/water/gas flow | Stone II k_ro, Corey k_rg, oil-gas Pc, explicit gas transport (experimental) |
| Corey relative permeability | S_wc, S_or, S_gc, S_gr; exponents n_w, n_o, n_g; maximums k_rw_max, k_ro_max, k_rg_max |
| Brooks-Corey capillary pressure | Oil-water (optional) and oil-gas (optional, 3-phase only) |
| Gravity segregation | Optional toggle, ρ·g·Δz head per phase |
| Peaceman well model | Rate or BHP control, dynamic PI per timestep |
| Well BHP constraints | Auto-switch rate→BHP if limit violated |
| Material balance tracking | Per-timestep MB error |
| PCG solver with convergence warning | Max 1000 iterations, residual check |
| Saturation-weighted compressibility | Per-cell c_t = ϕ(c_o·S_o + c_w·S_w + c_g·S_g) + c_r |
| Water or gas injection | Injector flag + `injectedFluid` parameter (`"water"` or `"gas"`) |

### Not Implemented / Deferred

| Feature | Priority |
|---------|----------|
| Aquifer boundary conditions | Medium |
| Horizontal / deviated wells | Medium |
| Non-uniform cell sizes | Medium |
| Leverett J-Function capillary scaling | Medium |
| Capillary hysteresis | Low |
| Per-cell capillary pressure variation | Low |

See [TODO.md](TODO.md) for the full work backlog and product roadmap.
