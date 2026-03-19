# ResSim — TODO

## Current Issues

- [ ] **LIMITATION: Stop latency = one WASM step duration** — The worker checks `stopRequested` between steps (`chunkYieldInterval: 1`). For large grids where a single step takes >500 ms, the Stop button has noticeable lag after clicking "Stopping…". Options if this becomes an issue:
  - **SharedArrayBuffer + Atomics**: main thread writes to shared memory, worker reads mid-step. Requires `Cross-Origin-Isolated` headers. Zero message latency.
  - **Worker termination + recreate**: immediate, but loses in-progress state.
  - **Rust-side callback hook**: check a flag inside the WASM step function every N solver iterations.
  - Current implementation is adequate for grids up to ~30×30×20 at <100 ms/step.

---

## Active Work

### ~~S1 — Scenario/Sensitivity Architecture Redesign~~ ✅ COMPLETE (2026-03-19)

Consolidated 18 scenarios → 8 canonical scenarios (6 + 2 gas); replaced single `sensitivity?` slot with `sensitivities: SensitivityDimension[]` array; multi-dimension sensitivity selection; domain grouping (Waterflood / Sweep / Depletion / Gas); `chartPresetOverride` per dimension; stale-key guard in UI; explicit params per scenario + `Object.freeze()`; `enabledByDefault?` on variants; all 28 test files pass.

See REFACTOR.md § Phase 2 for full design spec and canonical scenario map.

---

### Simplification Refactor — Step 7 Remainder

Eight legacy catalog/benchmark files still have active production dependencies and cannot yet be deleted (see REFACTOR.md Phase 1). The blocker is that `ReferenceExecutionCard.svelte`, `benchmarkRunModel.ts`, and the chart layer still import from `caseCatalog.ts`, `benchmarkCases.ts`, `caseLibrary.ts`, and `phase2PresetContract.ts`.

- [ ] Audit whether `ReferenceExecutionCard` and `benchmarkRunModel` are still needed once S1 lands, or whether the sweep run model supersedes them
- [ ] Delete confirmed-dead files; update any remaining imports; verify 0 TS errors and all tests pass

Files pending deletion: `ReferenceExecutionCard.svelte`, `benchmarkCases.ts`, `benchmarkRunModel.ts`, `benchmarkDisclosure.ts`, `caseCatalog.ts`, `caseLibrary.ts`, `presetCases.ts`, `phase2PresetContract.ts`

---

### F4 — Unify Chart and Output Architecture

Goal: one consistent interaction model for x-axis selection, panel expansion, legends, and output summaries across live runs and reference comparisons.

- [ ] Consolidate chart-shell header and expansion-state wiring
- [ ] Finish shared panel/x-axis selection across both chart types
- [ ] Resolve remaining Results card verbosity

Acceptance: chart behaviour feels consistent regardless of run type; future output features do not require parallel implementation in both chart shells.

Primary files: `src/lib/charts/RateChart.svelte`, `src/lib/charts/ReferenceComparisonChart.svelte`, `src/lib/charts/referenceComparisonModel.ts`

---

## Product Roadmap

### ~~F10 — Simulation Sweep Efficiency~~ ✅ COMPLETE (2026-03-19)

Simulation sweep efficiency (E_A_sim, E_V_sim, E_vol_sim) is now computed from `grid.sat_water` (per-cell saturation already streamed to the frontend for the 3D view). Three separate panels — Areal / Vertical / Volumetric — each show solid = simulation, dashed = analytical. Vertical panel only shown when nz > 1.

---

### F5 — Multi-Case Comparison Beyond Charts

- [ ] Case selection/switching for the 3D view across sensitivity runs
- [ ] Comparison awareness in saturation profile and compact summary cards
- [ ] Synchronized selected case across summary, chart, and 3D inspection

Acceptance: sensitivity studies can be inspected spatially, not only in charts.

---

### F6 — Compact Input Layout

- [ ] Reduce default section padding and vertical spacing
- [ ] Convert overly tall input groups into compact flowing cards where possible
- [ ] Tighten margins and whitespace without making the UI cramped

Acceptance: common scenario editing takes materially less scrolling on desktop; dense scientific inputs remain legible.

---

### F7 — Redesign Themes

- [ ] Replace near-black dark / flat-white light surfaces with deliberate working themes
- [ ] Remove or significantly soften the reservoir-layer page background treatment
- [ ] Improve panel contrast, content focus, and data-first visual balance

Acceptance: both themes feel designed for sustained technical use; decorative background no longer competes with data surfaces.

---

### F8 — Custom Mode Boundaries

After S1 the predefined scenario space will be substantially richer. Custom mode should read as intentional exploratory modelling, not a catch-all.

- [ ] Explicitly define what custom mode offers beyond the predefined scenarios
- [ ] If a user is redirected to custom, explain why
- [ ] Make the transition from a scenario to custom editing feel deliberate (clone + edit, not silent fallback)

Acceptance: "Custom" reads as a power-user feature; the scenario picker covers 95% of educational use without touching custom mode.

---

### F9 — Gas Scenarios

- [ ] Promote Gas Injection and Solution Gas Drive from experimental to production-ready
- [ ] Add analytical reference for 1D gas-oil displacement (Buckley-Leverett with gas properties)
- [ ] Wire Gas Injection sensitivity dimensions: mobility ratio, S_gc, permeability
- [ ] Fix confirmed physics bugs first (see Deferred — Physics Correctness Issues)

Acceptance: gas scenarios behave like the waterflood scenarios — scenario + sensitivity dimensions + analytical comparison.

---

### F10 — Refresh Docs After UI Pass

- [ ] Update README, BENCHMARK_MODE_GUIDE, DOCUMENTATION_INDEX after F4–F9 land
- [ ] Ensure docs describe the final workflow and terminology, not transitional states

---

## Deferred / Later

### Physics — Correctness Issues (3-Phase)

Confirmed bugs. Viscous-dominated 2-phase runs are unaffected; gravity-drainage and capillary-equilibrium studies are.

- [ ] **Gas-oil capillary pressure direction** (`capillary.rs` `GasOilCapillaryPressure`) — `P_cog` currently decreases as S_g increases. Physical requirement: gas is non-wetting, so Pc = P_gas − P_oil must increase with S_g. Fix: parameterise on `S_o_eff` using `Sorg` (see below).
- [ ] **Stone II missing `Sorg` parameter** (`relperm.rs`) — `k_ro_gas` uses `s_gr` (residual gas after water imbibition) as terminal oil saturation in a gas flood. These are distinct. Add `s_org` (residual oil to gas, typically > `s_or`) and wire through `k_ro_gas`, capillary pressure, and UI.
- [ ] **3-phase material-balance diagnostic tracks water only** (`step.rs`) — `actual_change_m3` accumulates only ΔSw × Vp. Add parallel accumulators for gas and oil so all three phases are covered.

### Physics — Known Limitations (Black-Oil Model)

Intentional simplifications documented here for clarity. Not bugs.

- [ ] **No bubble-point / dissolved-gas tracking** — Immiscible model only. Adding Rs(P), Bo(P) correlations would upgrade to full black-oil PVT.
- [ ] **Constant gas compressibility** — `c_g ≈ 1/P` for real gas via z-factor is not modelled. Constant-c overestimates gas compressibility at high pressure and underestimates at low pressure.
- [ ] **Constant fluid viscosity and density** — No PVT table. Error small for viscous-force-dominated waterflood at moderate pressure; larger for gas at varying pressure.
- [ ] **Immiscible, not compositional** — No phase equilibrium, no K-value flash. Correct scope for black-oil, but must be stated clearly.

### Physics Extensions

- [ ] Well schedule support
- [ ] Aquifer boundary conditions
- [ ] Per-cell or per-layer porosity variation
- [ ] Per-cell initial water saturation / transition-zone initialization
- [ ] Additional published benchmark families

### Benchmark and Comparison Tooling

- [ ] Grid-convergence study preset family
- [ ] A/B run comparison overlays
- [ ] Relative error (%) diagnostic curves
- [ ] Uncertainty and sensitivity batch runner beyond curated benchmark sensitivities

### Visualization and Charting

- [ ] Sw profile plot evolution and tighter 3D companion integration
- [ ] Cross-section / slice viewer for i/j/k inspection in the 3D view
- [ ] Summary statistics panel (OOIP, pore volume, RF, average pressure/saturation, water cut, VRR)

### Data I/O

- [ ] Structured scenario export/import
- [ ] CSV/JSON export of results and benchmark summaries

### Wells and Advanced Reservoir Modeling

- [ ] Multi-well patterns (5-spot, line-drive, custom placements)
- [ ] Non-uniform cell sizes and local grid refinement

### Nice To Have Only

- [ ] Benchmark trend tracking across CI runs
- [ ] Multi-chart synchronized zoom/pan
- [ ] Responsive/mobile chart and 3D layout improvements
- [ ] Phase relative permeability / capillary curve visualization
- [ ] Report export for plots and key metrics
- [ ] Undo/redo for parameter changes
- [ ] Horizontal or deviated well model with generalized Peaceman PI
- [ ] Per-cell capillary pressure variation and capillary hysteresis
- [ ] Fetkovich type-curve overlay expansion

---

## Completed

- **B1–B10** (2026-03-07): Benchmark modernization — family registry, explicit reference policy, sensitivity sweeps, benchmark-specific chart defaults, benchmark docs.
- **F1–F3** (2026-03): Unified Inputs/Run/Outputs shell; family-first navigation; case library; reference execution card; warning policy unified; case disclosure cards; compact Run Set selector; master-detail Results layout; IBM Plex Sans/Mono typography; semantic utility classes.
- **Store refactor** (2026-03-17): Converted `createSimulationStore()` from function-based getter/setter boilerplate to Svelte 5 class with `$state` fields. Fixed silent bug: 13 three-phase parameters declared as `$state` but never exposed in `parameterState` accessor.
- **Simplification Refactor Steps 1–6** (2026-03-17): `scenarios.ts` + `ScenarioPicker.svelte` replace `ModePanel.svelte` + 4-layer case-library navigation. Store wired. `evaluateAnalyticalStatus` moved to `warningPolicy.ts`. `buildScenarioNavigationState` removed from store. `ModePanel.svelte` deleted. All 204 tests pass.
- **Run Controls UX** (2026-03-19): Stop button shows "Stopping…" immediately; `stopPending` state added. Steps-reset bug on scenario run fixed (save/restore `this.steps` around `applyCaseParams`).
- **Sweep Efficiency** (2026-03-19): Analytical sweep efficiency module (`sweepEfficiency.ts`): Craig (1971) areal sweep, Dykstra-Parsons (1950) vertical sweep, volumetric product. `SweepEfficiencyChart.svelte` renders E_A, E_V, E_A × E_V curves. Four new sweep scenarios: Areal–Mobility, Areal–Residual, Vertical–V_DP, Combined Sweep.
- **S1 — Scenario/Sensitivity Architecture Redesign** (2026-03-19): Consolidated 18 scenarios → 8 canonical scenarios with multi-dimension sensitivity selection. New types `ScenarioDomain`, `SensitivityDimension`; `sensitivities: SensitivityDimension[]` replaces `sensitivity?`; domain grouping in ScenarioPicker; `chartPresetOverride` per dimension; stale-key guard; explicit params + `Object.freeze()`; `enabledByDefault?` on variants; `activeSensitivityDimensionKey` in store; `selectSensitivityDimension()` action; 28/28 tests pass.
