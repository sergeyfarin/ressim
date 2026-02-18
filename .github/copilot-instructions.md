# Copilot Instructions for ResSim

## Project Overview

ResSim is a browser-based 3D reservoir simulator combining:
- **Backend**: Rust core compiled to WebAssembly for high-performance numerical simulation
- **Frontend**: Svelte 5 + Vite for reactive UI, Three.js for 3D visualization, Chart.js for analytics
- **Focus**: Two-phase (oil/water) flow simulation with physics validation via Buckley-Leverett benchmarks

## Architecture

### Core Components
1. **Rust/WASM Simulator** (`src/lib/ressim/src/`)
   - `lib.rs` - Core simulation entry points, WASM bindings
   - `grid.rs` - 3D Cartesian grid management
   - `solver.rs` - Pressure solver using conjugate gradient method
   - `step.rs` - Saturation stepping and time integration
   - `capillary.rs` - Capillary pressure physics
   - `relperm.rs` - Relative permeability (SCAL)
   - `well.rs` - Well control and rate allocation

2. **Frontend** (`src/`)
   - `App.svelte` - Main UI, controls, playback, benchmark display
   - `lib/3dview.svelte` - Three.js 3D property visualization
   - `lib/RateChart.svelte` - Production rate charts with analytical comparison
   - `lib/sim.worker.js` - Web Worker bridge to keep UI responsive during simulation

3. **Build & CI**
   - `scripts/export-benchmarks.mjs` - Generate benchmark artifact JSON from Rust tests
   - `.github/workflows/` - Automated benchmark publishing and distribution builds

## Technology Stack

### Rust Dependencies (Cargo.toml)
- `wasm-bindgen` - JavaScript/Rust interop
- `nalgebra` - Dense linear algebra (vectors, CG solver)
- `sprs` - Sparse matrices for pressure system
- `serde`, `serde-wasm-bindgen` - Serialization
- `getrandom[wasm_js]` - WASM-compatible RNG

### JavaScript Dependencies (package.json)
- `svelte` 5.x - Reactive UI framework
- `vite` 7.x - Build tool
- `tailwindcss` 4.x + `daisyui` - Styling
- `three` 0.182.0 - 3D rendering (pinned version)
- `chart.js` 4.x - 2D charting

## Development Workflow

### Prerequisites
- Node.js + npm
- Rust toolchain (`rustup`)
- `wasm-pack` (`cargo install wasm-pack`)
- `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`)

### Common Commands
```bash
# Development
npm run dev                    # Builds WASM then starts Vite dev server
npm run build:wasm             # Build only WASM package
npm run build                  # Production build (includes bench:export)
npm run preview                # Preview production build

# Benchmarks & Artifacts
npm run bench:export           # Export benchmark JSON from Rust tests
npm run cases:export           # Export case configurations

# Rust-specific
cd src/lib/ressim
cargo test                     # Run Rust unit tests
cargo test benchmark_buckley_leverett -- --nocapture  # View benchmark details
cargo check                    # Quick type/syntax check
```

### Build Pipeline
1. `npm run dev` triggers `predev` hook → builds WASM first
2. `npm run build` generates `public/benchmark-results.json` before bundling
3. WASM output goes to `src/lib/ressim/pkg/` and is imported by JS

## Coding Conventions

### Rust Code
- Use idiomatic Rust: explicit error handling, no unwrap() in library code
- Follow standard formatting: `cargo fmt`
- Keep physics algorithms in dedicated modules (`capillary.rs`, `relperm.rs`, etc.)
- Document public APIs with `///` doc comments
- Use `#[wasm_bindgen]` for JavaScript-exposed functions

### Svelte Code
- Use Svelte 5 runes syntax (`$state`, `$derived`, `$effect`)
- Prefer TypeScript-style JSDoc comments for complex props
- Keep components focused and single-purpose
- Use Tailwind + DaisyUI classes for styling
- Store simulation state in reactive stores, not component state

### General Practices
- Keep changes minimal and focused
- Document breaking changes or API modifications in commit messages
- Update relevant docs in `docs/` for significant feature changes
- Preserve existing benchmark tolerances unless physics changes require adjustment

## Key Concepts

### Simulation Loop
1. User configures scenario (grid, SCAL, capillary params, wells)
2. Worker creates Rust simulator instance
3. `step()` calls solve pressure → advance saturation → update wells
4. Worker posts state snapshots back to UI for visualization

### Benchmarking
- Benchmarks validate physics against analytical Buckley-Leverett solution
- Implemented as Rust `#[test]` functions in `src/lib/ressim/src/lib.rs`
- Exported via `scripts/export-benchmarks.mjs` → `public/benchmark-results.json`
- Frontend displays benchmark results in UI table

### Legend Modes
- **Fixed**: Property-specific ranges for cross-run comparison
- **Percentile**: Adaptive ranges for emphasizing current data contrast

## File Structure Reference

```
ressim/
├── .github/
│   ├── workflows/           # CI/CD pipelines
│   └── copilot-instructions.md
├── src/
│   ├── main.js              # Entry point
│   ├── App.svelte           # Main UI
│   ├── lib/
│   │   ├── ressim/          # Rust WASM core
│   │   │   ├── src/         # Rust source modules
│   │   │   ├── Cargo.toml
│   │   │   └── pkg/         # Built WASM (gitignored)
│   │   ├── ui/              # Svelte UI components
│   │   ├── 3dview.svelte    # 3D visualization
│   │   ├── RateChart.svelte # Rate charts
│   │   └── sim.worker.js    # Web Worker bridge
├── public/
│   ├── cases/               # Scenario presets
│   └── benchmark-results.json  # Generated artifact
├── docs/                    # Extensive technical documentation
├── scripts/                 # Build automation
├── package.json
├── vite.config.js
└── README.md
```

## Testing

### Rust Tests
- Run `cargo test` in `src/lib/ressim/`
- Benchmark tests include tolerance checks against analytical solutions
- Use `--nocapture` flag to see detailed benchmark output

### Integration
- Benchmarks are smoke-tested by build process (`npm run bench:export`)
- Visual validation through UI: load scenarios and verify 3D/chart behavior

### Known Non-Blocking Warnings
- DaisyUI `@property` CSS compatibility warning
- Large JS chunk size warning (accepted for current project state)

## Documentation

Primary docs in `docs/` folder:
- `DOCUMENTATION_INDEX.md` - Complete documentation map
- `P4_TWO_PHASE_BENCHMARKS.md` - Benchmark methodology
- `P4_SUMMARY.md` - Phase 4 completion report
- Various CAPILLARY_*.md - Physics implementation details
- See `README.md` for quick start and FAQ

## Common Tasks

### Adding a New Physics Feature
1. Implement in appropriate Rust module (e.g., `relperm.rs`)
2. Add tests in module or `lib.rs`
3. Update WASM bindings if needed
4. Update UI controls in relevant Svelte components
5. Document in `docs/` if significant
6. Update benchmarks if validation required

### Modifying UI Components
1. Edit Svelte files in `src/lib/`
2. Use Tailwind classes for styling
3. Test with `npm run dev`
4. Ensure responsive design works across viewport sizes

### Updating Benchmarks
1. Modify test in `src/lib/ressim/src/lib.rs`
2. Run `cargo test` to verify
3. Run `npm run bench:export` to regenerate artifact
4. Rebuild frontend to see updated results

## Best Practices

1. **Minimal Changes**: Only modify files directly related to the task
2. **Preserve Physics**: Don't change validated benchmark tolerances without justification
3. **Build Verification**: Always run `npm run dev` and `cargo check` before committing
4. **Documentation**: Update docs for user-facing or API changes
5. **Dependencies**: Avoid adding new dependencies unless essential; use existing libraries
6. **Worker Safety**: Ensure worker-posted state snapshots remain serializable
7. **WASM Compatibility**: Test that Rust changes compile to WASM (`npm run build:wasm`)

## Known Constraints

- Three.js version pinned to 0.182.0 (breaking changes in newer versions)
- WASM build requires `wasm32-unknown-unknown` target
- Worker communication must use structured cloning (no functions/classes)
- Grid size impacts performance (current sweet spot: 20x20x10 for demos)

## Roadmap Context

From `TODO_2026.md`:
- **Completed**: P0-P4 phases including two-phase benchmarks
- **Near-term**: Consider 3-phase extension (oil/water/gas)
- **Nice-to-Have**: Aquifer coupling, ensemble runs, uncertainty analysis

When implementing features, align with roadmap priorities and preserve existing validated functionality.
