# ResSim — Black-Oil Reservoir Simulator with Analytical Comparison

Compare IMPES numerical solutions against classical analytical methods — Buckley-Leverett, Craig sweep, Dykstra-Parsons, Dietz decline — with scenario-based sensitivity sweeps and 3D visualization. Runs entirely in the browser via Rust/WASM.

## Current State (2026-03)

- **8 canonical scenarios** across Waterflood, Sweep, Depletion, and Gas domains, each with selectable sensitivity dimensions.
- **Sweep efficiency**: Craig (1971) areal, Dykstra-Parsons (1950) vertical, and combined volumetric sweep with recovery-factor overlay.
- **Two-phase oil/water**: validated against Buckley-Leverett theory (2.5–3.1% error on refined grids).
- **Three-phase oil/water/gas** (experimental): Stone II relative permeability implemented but has known correctness gaps (see Model Validity Notes).
- **All execution in-browser** via WASM — no backend required.

## Analytical Methods

| Scenario | Analytical Method | Reference |
|----------|-------------------|-----------|
| `wf_bl1d` — 1D Waterflood | Buckley-Leverett fractional-flow with Welge shock construction | Buckley & Leverett (1942); Welge (1952) |
| `sweep_areal` — Areal Sweep | Craig confined five-spot areal sweep correlation | Craig (1971); Dyes, Caudle & Erickson (1954) |
| `sweep_vertical` — Vertical Sweep | Dykstra-Parsons non-communicating layered sweep with BL displacement efficiency | Dykstra & Parsons (1950) |
| `sweep_combined` — Combined Sweep | Craig × Dykstra-Parsons × BL via local-PVI approximation | Craig; DP; BL; Welge |
| `dep_pss` — Pressure Depletion | Dietz pseudo-steady-state bounded-drainage decline | Dietz (1965) |
| `dep_decline` — Rate Decline | Exponential decline reference | Fetkovich (1971) |
| `gas_injection` — Gas Injection | *Planned: gas-oil Buckley-Leverett* | Simulation-only currently |
| `gas_drive` — Solution Gas Drive | *Planned: immiscible gas depletion analytics* | Simulation-only currently |

**Interpretation notes:**
- `sweep_areal` random-heterogeneity variants are simulation-dominant; Craig curve serves as baseline context.
- `sweep_combined` uses a first-order local-PVI approximation. Stiles layer-by-layer method is the planned upgrade (see `TODO.md` Phase 5).
- `dep_pss` well-location sensitivity currently changes only the simulation; the analytical helper does not yet consume producer position (see `TODO.md` Phase 1B).

## Features

### Simulation Engine (Rust → WASM)

- **IMPES solver** — implicit pressure, explicit saturation on a 3D Cartesian grid.
- **Two-phase oil/water** — Corey relative permeability, optional Brooks-Corey capillary pressure.
- **Three-phase oil/water/gas** (experimental) — Stone II k_ro, Corey k_rg, oil-gas capillary pressure, explicit gas transport, CFL extended to gas.
- **Corey relative permeability** — configurable endpoints (S_wc, S_or, S_gc, S_gr) and exponents (n_w, n_o, n_g).
- **Brooks-Corey capillary pressure** — optional oil-water and oil-gas curves with numerical cap at 20 × P_entry.
- **Gravity** — configurable toggle with phase-density-weighted hydrostatic head.
- **Permeability modes**: uniform, random (optional deterministic seed), per-layer CSV input.
- **Well model**: Peaceman PI, rate or BHP control, per-layer completion, auto BHP constraint switching, dynamic PI update.
- **Adaptive timestepping**: CFL-based saturation limit, pressure change limit, well rate change limit.
- **Material-balance error** tracking per timestep; PCG solver convergence warning.

### Reference Solutions & Comparisons

- **Buckley-Leverett**: Welge tangent construction, Sw profile, post-breakthrough outlet Sw via bisection, cumulative PVI tracking.
- **Depletion decline**: Dietz PSS with shape-factor table (square, rectangular, elongated); Peaceman PI with per-layer summation; 1D slab and 2D radial geometries.
- **Sweep efficiency**: Craig five-spot areal sweep (breakthrough + post-BT growth), Dykstra-Parsons layered vertical sweep, combined recovery factor.
- **Mismatch metrics**: MAE, RMSE, MAPE displayed in comparison charts.

### Visualization & UI

- **3D property view** (Three.js) — pressure, water/oil/gas saturation, permeability (x/y/z), porosity. Fixed / Percentile range modes.
- **Rate chart** — collapsible Rates, Cumulative, Diagnostics panels with 21 curves. X-axis modes: time, log-time, PVI, cumulative liquid/injection.
- **Scenario picker** — predefined scenarios with selectable sensitivity dimensions; custom mode for advanced exploration.
- **Sensitivity sweeps** — select dimension and variants; one simulation per variant; stored results feed comparison charts.
- **Sweep efficiency charts** — E_A, E_V, E_vol vs PVI with simulation and analytical overlays.
- **Worker-based stepping** keeps UI responsive. Replay/history controls with time slider.
- **Dark/Light theme** toggle.

### Model Validity Notes

- **Sweep recovery overlays are approximate.** The current formula combines Craig, Dykstra-Parsons, and BL through a local-PVI approximation — useful for qualitative comparison, not a substitute for Stiles or stream-tube methods.
- **Craig areal sweep is five-spot-specific.** Not applicable to line drives, peripheral floods, or other patterns.
- **Dykstra-Parsons assumes non-communicating layers (Kv = 0).** When vertical communication is significant, the analytical penalty is conservative vs. the simulator.
- **Three-phase mode remains experimental.** Known issues: gas-oil Pc direction inverted, Stone II missing S_org parameter, oil-phase material balance not tracked. See `TODO.md` Phase 1A.
- **Capillary pressure capped at 20 × P_entry** for numerical stability — not a physical plateau.
- **All PVT properties are constant** (no pressure dependence). Adequate for viscous-dominated waterfloods at moderate pressure; inaccurate near bubble point or for gas at varying pressure. Black-oil PVT extension planned in `TODO.md` Phase 4.

### Validation & Benchmarks

Buckley-Leverett breakthrough PVI benchmarks (Rust unit tests):

| Case | Analytical PV_BT | Simulator PV_BT | Relative Error |
|------|------------------:|----------------:|---------------:|
| BL-Case-A (favorable mobility) | 0.586 | 0.609 | 4.0% |
| BL-Case-B (adverse mobility) | 0.507 | 0.553 | 9.0% |

Refined discretization (nx=96, dt=0.125d) reduces errors to 2.5–3.1%. Current 25–30% acceptance limits are coarse regression guards; observed accuracy is much better.

## Scenarios

| Domain | Key | Sensitivity Dimensions |
|--------|-----|------------------------|
| Waterflood | `wf_bl1d` | Mobility ratio, Corey n_o, S_or, capillary, grid |
| Sweep | `sweep_areal` | Mobility ratio, areal heterogeneity, S_or |
| Sweep | `sweep_vertical` | V_DP heterogeneity, mobility ratio |
| Sweep | `sweep_combined` | Mobility × vertical heterogeneity |
| Depletion | `dep_pss` | Well location, skin, permeability, compressibility |
| Depletion | `dep_decline` | Skin, permeability |
| Gas | `gas_injection` | *Planned* |
| Gas | `gas_drive` | *Planned* |

## Roadmap

See [TODO.md](TODO.md) for the phased roadmap:

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Consolidate & fix (3-phase bugs, analytical gaps, legacy cleanup) | Active |
| 2 | Custom mode redesign & UX | Planned |
| 3 | Gas scenarios with analytical references | Planned |
| 4 | Black-oil PVT (volatile oil, gas cap, Rs/Bo) | Planned |
| 5 | Advanced analytics (Stiles, Warren-Root, multi-method) | Planned |
| 6 | Multi-case inspection & data I/O | Planned |
| 7 | Extended physics & well models | Future |

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
    ├── analytical/                 — Buckley-Leverett, depletion, sweep efficiency (TS + Svelte + tests)
    ├── charts/                     — RateChart, comparison charts, sweep charts, chart helpers/tests
    ├── visualization/              — Three.js 3D grid rendering + legend
    ├── workers/                    — Web Worker bridge to WASM simulator
    ├── simulator-types.ts          — TypeScript interfaces for worker payloads
    ├── buildCreatePayload.ts       — payload builder + tests
    ├── catalog/                    — scenarios.ts (primary) + legacy catalog code (pending cleanup)
    ├── ui/                         — ScenarioPicker, feedback surfaces, controls, cards, section panels
    ├── ui/controls/                — primitive UI controls (Button, Card, Input, Select, Collapsible)
    └── ressim/src/                 — Rust simulator core
        ├── lib.rs                  — WASM API surface + benchmark tests
        ├── step.rs                 — IMPES timestep logic (2-phase and 3-phase)
        ├── solver.rs               — PCG pressure solver (Jacobi preconditioned)
        ├── relperm.rs              — Corey rel-perm (2-phase) + Stone II (3-phase)
        ├── capillary.rs            — Brooks-Corey capillary pressure (oil-water + oil-gas)
        ├── well.rs                 — Peaceman well model + validation
        └── grid.rs                 — grid cell definitions + transmissibility
docs/                               — technical reference docs
REFACTOR.md                         — architecture decisions and design specs
TODO.md                             — phased roadmap and work items
```

## Key Documentation

| Document | Content |
|----------|---------|
| [docs/DOCUMENTATION_INDEX.md](docs/DOCUMENTATION_INDEX.md) | Map of authoritative docs |
| [docs/UNIT_SYSTEM.md](docs/UNIT_SYSTEM.md) | Comprehensive unit system and physics equations |
| [docs/P4_TWO_PHASE_BENCHMARKS.md](docs/P4_TWO_PHASE_BENCHMARKS.md) | BL benchmark methodology, tolerances, and results |
| [docs/THREE_PHASE_IMPLEMENTATION_NOTES.md](docs/THREE_PHASE_IMPLEMENTATION_NOTES.md) | Three-phase (Stone II) architecture and known gaps |
| [docs/IMPLEMENTATION_REVIEW_2026-03-19.md](docs/IMPLEMENTATION_REVIEW_2026-03-19.md) | Verified scientific gaps and recommended follow-ups |
| [docs/TRANSMISSIBILITY_FACTOR.md](docs/TRANSMISSIBILITY_FACTOR.md) | Derivation of `8.527×10⁻³` constant |
| [REFACTOR.md](REFACTOR.md) | Active architecture work and design decisions |
| [TODO.md](TODO.md) | Phased roadmap and prioritized work items |

## Physics Summary

### Implemented ✅

| Feature | Details |
|---------|---------|
| Two-phase oil/water flow | IMPES pressure-saturation splitting; validated |
| Three-phase oil/water/gas flow | Stone II k_ro, Corey k_rg, gas Pc, gas transport (experimental) |
| Corey relative permeability | S_wc, S_or, S_gc, S_gr; exponents n_w, n_o, n_g |
| Brooks-Corey capillary pressure | Oil-water + oil-gas (optional); 20× entry cap |
| Gravity segregation | Optional; ρ·g·Δz head per phase |
| Peaceman well model | Rate or BHP control; dynamic PI; per-layer completion |
| Material balance tracking | Per-timestep MB error (water phase; gas/oil planned) |
| PCG solver | Jacobi preconditioned; 1000 max iterations; convergence warning |
| Adaptive timestepping | CFL saturation, pressure, and well-rate limits |

### Planned

| Feature | Phase | Priority |
|---------|-------|----------|
| Gas-oil BL analytical | 3 | High |
| Black-oil PVT: Rs(P), Bo(P), Bg(P), μ(P) | 4 | High |
| Bubble-point tracking & phase split | 4 | High |
| Gas cap expansion/secondary gas cap | 4 | High |
| Stiles sweep method | 5 | Medium |
| Arps hyperbolic decline | 4 | Medium |
| Material-balance (Havlena-Odeh) | 4 | Medium |
| p/z diagnostic | 3 | Medium |
| Multi-well patterns | 7 | Lower |
| Aquifer boundary conditions | 7 | Lower |
| Non-uniform cell sizes | 7 | Lower |
| Horizontal/deviated wells | 7 | Lower |

See [TODO.md](TODO.md) for the full phased roadmap.
