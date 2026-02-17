# ResSim Gap Analysis & High-Value Improvements

Full-repo review of the 3D two-phase (oil/water) reservoir simulator with Buckley-Leverett analytical comparison. Excludes items already in `TODO_2026.md`.

---

## 1 — Simulation Core (Rust/WASM)

### 🔴 Critical Gaps

| # | Gap | Impact | Detail |
|---|-----|--------|--------|
| C1 | **Total compressibility ignores saturation weighting** | Incorrect pressure solution | [lib.rs:668](file:///home/reken/Repos/ressim/src/lib/ressim/src/lib.rs#L668): `c_t = c_o + c_w + c_r` should be `ϕ(c_o·S_o + c_w·S_w) + c_r` per cell. Currently treats compressibility as uniform and independent of saturation, producing wrong accumulation terms. |
| C2 | **Transmissibility factor 0.001127 is for bbl/day/psi, not m³/day/bar** | Systematic rate error | The constant `0.001127` in [lib.rs:527](file:///home/reken/Repos/ressim/src/lib/ressim/src/lib.rs#L527) originates from US oilfield units. For m³/day/bar with k in mD, the correct constant is `8.527×10⁻⁵`. The well PI calculation also uses this factor at [lib.rs:430](file:///home/reken/Repos/ressim/src/lib/ressim/src/lib.rs#L430). While benchmark tolerances still pass, absolute flow rates are off by ~13×. |
| C3 | **No material-balance error tracking** | Cannot detect bugs | There is no cumulative material-balance error computed or reported. For a two-phase IMPES simulator this is the single most important diagnostic to catch numerical drift. |

### 🟡 Moderate Gaps

| # | Gap | Impact |
|---|-----|--------|
| M1 | **Uniform porosity across grid** | Porosity is set once and identical for every cell. No per-cell or per-layer porosity input exists. |
| M2 | **Wells only connect through all layers (nz vertical completions)** | Worker always opens a perforation in every k-layer ([sim.worker.js:151-158](file:///home/reken/Repos/ressim/src/lib/sim.worker.js#L151-L158)). Cannot model partial completions or horizontal wells. |
| M3 | **No well PI update during simulation** | PI is computed once at `add_well()` from initial cell mobility. As saturation evolves, PI should be updated each step (total mobility changes). |
| M4 | **PCG solver has no convergence feedback** | If the solver does *not* converge in 1000 iterations, the last iterate is silently returned. No warning is raised. Adding a convergence flag or iteration count report would catch instabilities early. |
| M5 | **Gravity head uses total density at face, not phase-density split** | Gravity should act per-phase via phase potentials, not via a single total-density gravity head applied to total flux. Acceptable simplification for weak gravity, but physically incorrect for strong density contrast (e.g., oil/water with gas cap). |
| M6 | **No boundary conditions** | All reservoir boundaries are sealed (no-flow). Missing: constant-pressure boundary, aquifer influx (Carter-Tracy or Fetkovich), and periodic boundaries. |

---

## 2 — Analytical / Validation Layer

### High-Value Additions

| # | Item | Value |
|---|------|-------|
| A1 | **Material-balance check displayed on chart** | Add cumulative injection − cumulative production − in-place change as a numerical error series on `RateChart`. Immediate credibility indicator. |
| A2 | **Recovery factor (RF) vs PV-injected curve** | Standard industry plot absent from current charts. Easy to compute from existing `rateHistory`. |
| A3 | **Water cut vs PV-injected overlay** | Buckley-Leverett analytical water cut is computed in `FractionalFlow.svelte` but not plotted in the chart as an explicit curve vs PVI. |
| A4 | **Grid-convergence study preset** | Auto-run the same scenario at nx = 12, 24, 48, 96 and overlay results. Demonstrates numerical convergence without manual reruns. |
| A5 | **Welge tangent line visualisation** | `FractionalFlow.svelte` computes the shock front but does not render the fractional-flow curve or tangent line. A small f(Sw) chart would make the BL theory visually intuitive. |
| A6 | **Expanded benchmarks: 3D gravity, capillary, heterogeneous** | Current benchmarks are 1D BL-only. Adding at least one 3D gravity-drainage case and one heterogeneous 2D case would significantly improve validation coverage. |

---

## 3 — Frontend / Visualization

### High-Value Additions

| # | Item | Value |
|---|------|-------|
| V1 | **Cross-section / slice view** | 3D instanced-mesh view is powerful but for large grids it's hard to see interior cells. Add an i/j/k slice plane or clipping box. |
| V2 | **Fluid contact / saturation profile 1D plot** | Plot Sw vs distance along a user-selected row (e.g., the injector-producer axis). Essential for comparing flood front to BL theory. |
| V3 | **Export simulation results** | No CSV/JSON export of rate history, grid state, or saturation profiles exists. Users cannot post-process data externally. |
| V4 | **Undo / parameter comparison** | No way to compare two runs side-by-side. An A/B comparison mode or overlay of previous run on the rate chart would be valuable. |
| V5 | **Well schedule table (multi-rate)** | Currently only a single constant injection/production rate per run. Adding time-varying rate schedules (or at least a mid-run rate change) would unlock much richer scenarios. |
| V6 | **Responsive layout / mobile support** | The current layout uses `grid grid-cols-2 lg:grid-cols-3`-style structures but the 3D canvas + rate chart don't adapt gracefully on narrow screens. |
| V7 | **Progress bar for long runs** | During `workerRunning`, only a spinner text is shown. A progress bar with step count / ETA would improve UX for multi-hundred-step runs. |

---

## 4 — Code Quality & Architecture

| # | Item | Detail |
|---|------|--------|
| Q1 | **App.svelte is 1467 lines** | The monolithic root component holds *all* state, presets, validation, worker messaging, history, and lifecycle logic. Extract a `SimulationStore` or context module to manage shared state, reducing coupling. |
| Q2 | **No automated frontend tests** | There are zero Svelte/JS test files. At minimum, unit tests for `FractionalFlow` analytical calculations and `validateInputs()` logic would prevent regressions. |
| Q3 | **Worker payload shape is implicit** | The `configureSimulator()` function in `sim.worker.js` uses dynamic property access with `typeof X === 'function'` guards everywhere, making the contract fragile. A TypeScript interface shared between App and Worker would eliminate this. |
| Q4 | **Documentation index is stale** | [DOCUMENTATION_INDEX.md](file:///home/reken/Repos/ressim/docs/DOCUMENTATION_INDEX.md) still lists capillary pressure and gravity as "to be implemented" even though both are done. Several sections reference files that have moved. |
| Q5 | **No CI pipeline** | No GitHub Actions workflow runs `cargo test` or `npm run build` on push. The `.github/` directory has 2 files but no test workflow. |
| Q6 | **Benchmark JSON is checked in as a static artifact** | `public/benchmark-results.json` is generated by `npm run bench:export` but appears to be committed. Ideally, CI regenerates it and verifies against a baseline. |
| Q7 | **Rust code is a single 1856-line `lib.rs` file** | Consider splitting into modules: `grid.rs`, `well.rs`, `solver.rs`, `relperm.rs`, `step.rs`, `tests/`. |

---

## 5 — Physics Enhancements (Ordered by Impact)

| Priority | Item | Effort | Value |
|----------|------|--------|-------|
| ★★★ | **Per-cell porosity and porosity input** | Low | Unlocks heterogeneous reservoir models |
| ★★★ | **Correct unit constant (0.001127 → 8.527e-5)** | Low | Fixes absolute rate magnitudes |
| ★★★ | **Saturation-weighted compressibility** | Low | Correct accumulation term |
| ★★☆ | **Dynamic PI update each timestep** | Medium | Correct well behavior after water breakthrough |
| ★★☆ | **Per-cell initial saturation (Sw₀)** | Low | Transition zone, water-oil contact initialization |
| ★★☆ | **Constant-pressure / aquifer boundary** | Medium | Realistic reservoir boundary conditions |
| ★★☆ | **Well max/min BHP constraints in rate mode** | Low | Prevent vacuum or fracture-pressure violations |
| ★☆☆ | **Horizontal / deviated well model** | High | Peaceman-generalized PI for horizontal completions |
| ★☆☆ | **Variable cell sizes (non-uniform dx/dy/dz)** | High | Local grid refinement around wells |
| ★☆☆ | **Three-phase (gas cap / solution gas)** | Very High | Already noted as NTH-2 |

---

## Summary: Top 5 Quick Wins

1. **Fix saturation-weighted compressibility** — one-line change per cell in `calculate_fluxes()`
2. **Add material-balance error to rate chart** — ≤30 LoC in `RateChart.svelte`
3. **Per-cell porosity input** — add porosity array to worker payload + setter in Rust
4. **CI pipeline with `cargo test` + `npm run build`** — one GitHub Actions YAML
5. **Welge / f(Sw) plot** — small Chart.js canvas in `FractionalFlow.svelte` for analytical visualisation
