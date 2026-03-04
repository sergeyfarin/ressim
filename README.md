# ResSim — Browser-Based 3D Reservoir Simulator

A two-phase (oil/water) IMPES reservoir simulator built with **Rust/WASM** (physics core) and **Svelte 5 + Vite** (frontend). It provides interactive 3D visualization, production-rate charting with analytical comparisons, and benchmark-based validation — all running entirely in the browser.

## Features

### Simulation Engine (Rust → WASM)

- **IMPES solver** — implicit pressure, explicit saturation on a 3D Cartesian grid.
- **Corey relative permeability** model with configurable endpoints (`S_wc`, `S_or`) and exponents (`n_w`, `n_o`).
- **Brooks-Corey capillary pressure** — optional toggle, `P_entry` and `lambda` parameters.
- **Gravity** — configurable toggle with phase-density-weighted hydrostatic head.
- **Permeability modes**: uniform, random (optional deterministic seed), per-layer CSV input.
- **User-editable porosity**, initial water saturation, and rock compressibility.
- **Well model**: Peaceman PI, rate or BHP control, per-layer completion, auto BHP constraint switching. Dynamic PI update each timestep.
- **Material-balance error** tracking returned per timestep.
- **PCG solver convergence** warning surfaced in the UI.
- **Modular Rust layout**: `lib.rs` (WASM API), `step.rs`, `solver.rs`, `relperm.rs`, `capillary.rs`, `well.rs`, `grid.rs`.

### Analytical Comparisons

- **Buckley-Leverett** fractional-flow analytical curve for waterflood cases.
  - Welge tangent construction, Sw profile, post-breakthrough outlet Sw via bisection.
  - Breakthrough via cumulative PVI tracking.
- **Depletion decline** — PSS exponential decline `q(t) = q₀·exp(−t/τ)` with Dietz shape-factor PI.
  - 1D slab and 2D radial drainage geometry support.
  - Per-layer PI summation for multi-layer cases.
- Mismatch metrics: MAE, RMSE, MAPE displayed in the rate chart.

### Visualization & UI

- **3D property view** (Three.js) — selectable properties: pressure, water/oil saturation, permeability (x/y/z), porosity. Interactive legend with Fixed / Percentile range modes.
- **Rate chart** — collapsible Rates, Cumulative, Diagnostics panels with 21 curves. X-axis modes: time, log-time (Fetkovich), PVI, cumulative liquid/injection.
- **Sw Profile chart** — cell-index saturation profile compared to analytical flood front.
- **Scenario catalog (Demo Case Framework)** — 92 pre-calculated preset cases exploring geometry, heterogenty, physics, fluid properties, and relative permeability via a fast, faceted "compact flowing cards" UX.
- **Worker-based stepping** keeps UI responsive. Replay/history controls with time slider.
- **Simulation progress indicator** (step X / N).
- **Dark/Light theme** toggle.
- **Benchmark results card** — reads from `public/benchmark-results.json`, shows Baseline vs Refined discretization evidence.

### Validation & Benchmarks

Buckley-Leverett breakthrough PVI benchmarks (Rust unit tests):

| Case | Analytical PV_BT | Simulator PV_BT | Relative Error |
|------|------------------:|----------------:|---------------:|
| BL-Case-A (favorable mobility) | 0.586 | 0.524 | 10.6% |
| BL-Case-B (adverse mobility) | 0.507 | 0.477 | 6.0% |

Refined discretization (nx=96, dt=0.125d) reduces errors to 3–4%, confirming numerical diffusion as primary mismatch source.

Artifact pipeline: `npm run bench:export` → `public/benchmark-results.json`, consumed by frontend and CI.

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
| Transmissibility factor | `8.527×10⁻⁵` (DARCY_METRIC_FACTOR) |

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
npm run build      # runs bench:export → build:wasm → vite build
npm run preview    # serve production bundle locally
```

### Tests

```bash
npm test                           # Vitest frontend tests
cd src/lib/ressim && cargo test    # Rust unit + benchmark tests
```

> **Known non-blocking warnings**: DaisyUI `@property` CSS warning, large JS chunk warning.
> `src/lib/ressim/pkg/simulator.js` is wasm-pack generated — do not convert manually.

## Project Layout

```
src/
├── App.svelte                      — main UI: state, controls, scenario management
├── app.css                         — global styles
├── main.ts                         — app entry point
└── lib/
    ├── sim.worker.ts               — WebWorker bridge to WASM simulator
    ├── simulator-types.ts          — TypeScript interfaces for worker payloads
    ├── buildCreatePayload.ts       — payload builder + tests
    ├── caseCatalog.ts              — 10+ scenario presets with params + pre-run data
    ├── chart-helpers.ts            — chart data/config utilities + tests
    ├── RateChart.svelte            — rate/cumulative/diagnostics charting
    ├── ChartSubPanel.svelte        — reusable collapsible chart panel
    ├── SwProfileChart.svelte       — 1D saturation profile chart
    ├── FractionalFlow.svelte       — BL analytical computation
    ├── DepletionAnalytical.svelte  — PSS depletion analytical
    ├── 3dview.svelte               — Three.js 3D grid rendering + legend
    ├── ui/                         — TopBar, RunControls, TabContainer, InputsTab, panels
    ├── components/                 — ToggleGroup, PropertyToggle, etc.
    └── ressim/src/                 — Rust simulator core
        ├── lib.rs                  — WASM API surface
        ├── step.rs                 — IMPES timestep logic
        ├── solver.rs               — PCG pressure solver
        ├── relperm.rs              — Corey relative permeability
        ├── capillary.rs            — Brooks-Corey capillary pressure
        ├── well.rs                 — well model + validation
        └── grid.rs                 — grid cell definitions
scripts/
├── export-benchmarks.mjs           — benchmark artifact generation
├── export-cases.mjs                — pre-run case export
└── build-wasm.sh                   — WASM build script
public/
└── benchmark-results.json          — generated benchmark summary
docs/                               — technical reference docs (see below)
```

## Key Documentation (in `docs/`)

| Document | Content |
|----------|---------|
| `P4_TWO_PHASE_BENCHMARKS.md` | BL benchmark methodology, tolerances, and results |
| `UNIT_SYSTEM.md` | Comprehensive unit system reference |
| `UNIT_REFERENCE.md` | Quick unit lookup card |
| `TRANSMISSIBILITY_FACTOR.md` | Derivation of `8.527×10⁻⁵` constant |
| `ANALYTICAL_REVIEW.md` (root) | Full audit of analytical vs simulator physics |
| `CHART_IMPROVEMENTS.md` (root) | Chart architecture & curve reference |

## Physics Summary

### Implemented ✅

| Feature | Details |
|---------|---------|
| Two-phase oil/water flow | IMPES pressure-saturation splitting |
| Corey relative permeability | Configurable S_wc, S_or, n_w, n_o, k_rw_max, k_ro_max |
| Brooks-Corey capillary pressure | Optional, P_entry + lambda (with scaled physical caps) |
| Gravity segregation | Optional toggle, ρ·g·Δz head |
| Peaceman well model | Rate or BHP control, dynamic PI |
| Well BHP constraints | Auto-switch rate→BHP if limit violated |
| Material balance tracking | Per-timestep MB error |
| PCG solver with convergence warning | Max 1000 iterations, residual check |
| Saturation-weighted compressibility | Per-cell c_t = ϕ(c_o·S_o + c_w·S_w) + c_r |
| Injection of 100% water | Injector flag controls fluid composition |

### Not Implemented

| Feature | Priority |
|---------|----------|
| Three-phase flow (oil/water/gas) | Long-term |
| Aquifer boundary conditions | Medium |
| Horizontal / deviated wells | Medium |
| Non-uniform cell sizes | Medium |
| Leverett J-Function capillary scaling | Medium |
| Capillary hysteresis | Low |
| Per-cell capillary pressure variation | Low |
| Relative permeability endpoint scaling (k_rw_max, k_ro_max) | **Done** |

See [TODO.md](TODO.md) for the full list of remaining work items.
