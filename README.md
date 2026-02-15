# ResSim: Browser-Based Reservoir Simulator

ResSim is a 3D browser reservoir simulation app that combines a Rust/WASM simulation core with a Svelte + Vite frontend. It focuses on practical two-phase (oil/water) flow experimentation, interactive visualization, and benchmark-based credibility checks.

## What the app does

- Simulates pressure and saturation evolution on a 3D Cartesian grid.
- Supports configurable rock/fluid properties, capillary pressure, gravity toggle, and permeability scenarios.
- Renders a real-time 3D property view (Three.js), production-rate trends, and Buckley-Leverett analytical comparison.
- Records simulation history and supports replay for time-step inspection.
- Publishes benchmark summaries from generated artifacts so frontend values stay in sync with tests.

## Main features

### Simulation & physics

- Rust core compiled to WebAssembly (`src/lib/ressim`).
- SCAL controls (`S_wc`, `S_or`, Corey exponents), capillary pressure (`P_entry`, `lambda`), and gravity option.
- Permeability setup modes:
	- `default`
	- `random` (optional deterministic seed)
	- `perLayer` (layer-wise CSV input)

### Visualization & analysis

- 3D grid view with selectable properties:
	- Pressure
	- Water/Oil saturation
	- Permeability (`x`, `y`, `z`)
	- Porosity
- Rate chart with analytical Buckley-Leverett comparison.
- Scenario presets for reproducible demonstrations.
- Worker-based simulation stepping to keep UI responsive.

### Validation & credibility

- P4 Buckley-Leverett benchmark coverage with documented tolerances.
- Artifact pipeline: benchmark JSON is generated from test output and loaded by frontend.
- Baseline/Refined benchmark mode comparison shown in-app.

See:

- [docs/P4_TWO_PHASE_BENCHMARKS.md](docs/P4_TWO_PHASE_BENCHMARKS.md)
- [docs/P4_SUMMARY.md](docs/P4_SUMMARY.md)
- [docs/DOCUMENTATION_INDEX.md](docs/DOCUMENTATION_INDEX.md)

## Legend mode (Fixed vs Percentile)

In the **Visualization** panel:

1. Choose **Legend Range Mode**.
2. Use **Fixed** for stable, property-specific ranges (best for cross-run comparisons).
3. Use **Percentile (adaptive)** to emphasize contrast in the current dataset.
4. When using percentile mode, set **Low Percentile** and **High Percentile** bounds.

Defaults:

- Mode: `Fixed`
- Percentile bounds (when enabled): `P5` to `P95`

Legend labels now include:

- Clear property name
- Unit (`bar`, `mD`, or `fraction`)
- Active mode (`Fixed` or selected percentile window)

## Quick start

### Prerequisites

- Node.js + npm
- Rust toolchain (`cargo`)
- `wasm-pack`

### Install

```bash
npm install
```

### Run locally

```bash
npm run dev
```

### Build production

```bash
npm run build
```

Notes:

- `npm run build` runs `bench:export` first, generating `public/benchmark-results.json`.
- Known non-blocking warnings: DaisyUI CSS `@property` compatibility warning and large JS chunk warning.

## FAQ / Troubleshooting

- **What does `npm run dev` do?**
	- npm runs `predev` first (`npm run build:wasm`), then starts Vite:
		```bash
		npm run dev
		```
	- No Bun runtime is required.

- **`wasm-pack: command not found`**
	- Install via Cargo:
		```bash
		cargo install wasm-pack
		```
	- Confirm installation:
		```bash
		wasm-pack --version
		```

- **WASM build fails due to missing target**
	- Add the required Rust target:
		```bash
		rustup target add wasm32-unknown-unknown
		```

- **Rust toolchain not found (`cargo` missing)**
	- Install Rust using `rustup` from https://rustup.rs/.
	- Re-open terminal and verify with:
		```bash
		cargo --version
		```

- **Large chunk warning during `npm run build`**
	- This is currently expected and non-blocking for this project state.

- **DaisyUI `@property` CSS warning**
	- This is also expected in current build logs and does not block output generation.

## Useful commands

```bash
# Build WASM package only
npm run build:wasm

# Export benchmark artifact JSON
npm run bench:export

# Preview production build
npm run preview

# Run benchmark test directly from Rust core
cd src/lib/ressim
cargo test benchmark_buckley_leverett -- --nocapture
```

## Project layout

- `src/App.svelte` — main UI controls, playback, benchmark table.
- `src/lib/3dview.svelte` — Three.js reservoir rendering + legend.
- `src/lib/RateChart.svelte` — production/analytical charting.
- `src/lib/FractionalFlow.svelte` — analytical Buckley-Leverett support.
- `src/lib/sim.worker.js` — worker bridge between UI and WASM simulator.
- `src/lib/ressim/src/lib.rs` — Rust simulator core.
- `scripts/export-benchmarks.mjs` — benchmark artifact generation.
- `public/benchmark-results.json` — generated benchmark summary consumed by frontend.

## Current status & roadmap summary

From `TODO_2026.md`:

- Completed: P0 and P1 must-haves, all P3 items, and all P4 items (`P4-1` to `P4-5`).
- Recently completed: adaptive legend mode (`P4-4`) and benchmark artifact automation (`P4-5`).
- Open long-horizon items:
	- `NTH-2`: extend to three-phase flow (oil/water/gas)
	- `NTH-3`: optional aquifer coupling model

For full details, see [TODO_2026.md](TODO_2026.md).
