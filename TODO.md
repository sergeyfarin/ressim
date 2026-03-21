# ResSim — Roadmap

Phased plan to evolve ResSim from a validated 2-phase waterflood teaching tool into a full black-oil simulator with gas, volatile-oil, and compositional-aware analytics.

---

## Phase 1 — Consolidate & Fix (current foundations)

Goal: clean up tech debt, fix known correctness issues, finish incomplete work.

### 1A. Three-Phase Correctness Fixes

Confirmed bugs blocking gas scenarios from leaving "experimental" status.

- [x] **Gas-oil capillary pressure direction** — `capillary.rs`: P_cog now parameterised on S_o_eff using `s_org`; increases correctly with S_g.
- [x] **Stone II missing `S_org` parameter** — `relperm.rs`: `k_ro_gas` now uses `s_org` (residual oil to gas) as terminal saturation. `s_org` added to `RockFluidPropsThreePhase`, WASM API, TypeScript types, store, worker, and gas scenarios (default 0.20).
- [x] **3-phase material-balance diagnostic** — `step.rs`: gas accumulator (`actual_change_gas_m3`) added; gas injection/production tracked separately; `cumulative_mb_gas_error_m3` reported in `TimePointRates.material_balance_error_gas_m3`.

### 1B. Analytical Contract Gaps

- [x] **Dietz well-location sensitivity** — `computeShapeFactor()` now accepts `producerI`/`producerJ` and uses log-linear interpolation between tabulated C_A endpoints (center 30.8828, corner 0.5598). All three adapter call sites pass grid dims and producer position. `affectsAnalytical: true` for both shape-factor variants. Tests prove center/corner divergence in q0 and tau.
- [x] **Analytical adapter coverage tests** — contract test in `scenarios.test.ts` verifies every `affectsAnalytical: true` variant actually changes analytical output. Runs BL, sweep, and depletion fingerprints for all 6 analytical scenarios (50 test cases).
- [x] **Analytical method disclosure upkeep** — `dep_pss` summary already reads "for the active well location"; `dep_decline` (1D slab, no position sensitivity) is also correct. `ScenarioPicker.svelte` renders metadata directly — no alignment gap.

### Discoveries (from 1B work)

- **`sweep_ladder` affectsAnalytical semantic gap** — `sweep_combined / sweep_ladder` variants patch `mu_o` (which DOES change BL/sweep analytical output) yet are marked `affectsAnalytical: false`. This is intentional — the ladder dimension uses a shared analytical reference for pedagogical clarity while degradation is shown via simulation only. The contract test correctly skips the `false` direction for this reason. Document this pattern if more "intentionally-shared" dimensions are added.
- **Pre-existing test failures** — 7 UI/terminology tests (`modePanelFlows`, `outputTerminology`, `terminologyCopy`, `appThemeTypography`, `appStoreDomainWiring`, `ratechart-usage`) and 2 `referenceComparisonModel` tests fail on clean master. Not caused by 1B changes.
- **Dietz C_A log compression** — the 55× shape factor ratio (center vs corner) compresses to only ~30% PI difference through the `ln(A/C_A)` denominator. This limits the visual divergence between analytical curves; the scenario description should set appropriate expectations.

### 1C-caps. Scenario Capability Declarations (Phase 1 Refactor)

`ScenarioCapabilities` type added to `Scenario` — each scenario explicitly declares its analytical method, primary rate curve, sweep panel, x-axis, injector presence, 3D default, and three-phase gate. Consumer code reads capabilities instead of branching on `scenarioClass`/`domain`. All consumer-side branching now uses `analyticalMethod` (unified vocabulary); `caseMode` eliminated.

**Completed (2026-03-21):**
- [x] `ScenarioCapabilities` type with 9 behavioral fields replaces scattered conditional logic
- [x] All 8 scenarios split into per-scenario files under `src/lib/catalog/scenarios/` for easy comparison
- [x] `scenarios.ts` restructured as barrel file (types + chart presets + imports + lookup helpers)
- [x] `CUSTOM_MODE_CAPABILITIES` default for custom mode (no scenario object)
- [x] `simulationStore.svelte.ts`: `activeScenarioAsFamily` adapter, `CaseMode` resolution, sweep specs all use capabilities
- [x] `App.svelte`: `showSweepPanel`, `default3DProperty` use capabilities
- [x] `ScenarioPicker.svelte`: `requiresThreePhaseMode` replaces `domain !== 'gas' || activeMode === '3p'`
- [x] `referenceComparisonModel.ts`: sweep panel gate simplified to `family.showSweepPanel`
- [x] `ReferenceComparisonChart.svelte`: sweep panel gate and gas-oil-bl preview axis fixed

**Consumer-side branching — migrated to `analyticalMethod` (2026-03-21):**
- [x] All consumer types (`BenchmarkRunSpec`, `BenchmarkRunResult`, `BenchmarkReferencePolicy`) use `analyticalMethod: AnalyticalMethod` instead of `scenarioClass`
- [x] All branching in `benchmarkRunModel.ts`, `referenceComparisonModel.ts`, `referenceChartConfig.ts`, `benchmarkDisclosure.ts`, `ReferenceComparisonChart.svelte` uses `analyticalMethod`
- [x] `previewScenarioClass` prop/param renamed to `previewAnalyticalMethod` throughout
- [x] `phase2PresetContract.ts` `benchmarkScenarioClass` parameter type widened to `AnalyticalMethod`
- [x] `caseMode` eliminated from `ScenarioCapabilities` — derived from `analyticalMethod` at the single consumption site in `simulationStore`
- [x] `BenchmarkScenarioClass` type and `BenchmarkFamily.scenarioClass` field kept as deprecated — only used by the 5 benchmark family definitions and the adapter mapping in `simulationStore`

**Discovered issues:**
- `gas_injection` scenario was silently falling through to `CaseMode: 'dep'` instead of `'3p'` — now CaseMode derived from analyticalMethod (gas-oil-bl → 'dep')
- `gas_drive` same issue — CaseMode derived as 'dep' (analyticalMethod: 'none')
- `ReferenceComparisonChart.svelte` preview x-axis effect was missing `'gas-oil-bl'` — gas-oil scenarios in preview mode weren't defaulting to PVI x-axis (fixed)

### 1C. Legacy Cleanup (Phase 1 Step 7 Remainder)

Eight legacy catalog/benchmark files still have active production dependencies. The blocker is that `ReferenceExecutionCard`, `benchmarkRunModel`, and the chart layer still import from old catalog files.

- [x] Audit whether `ReferenceExecutionCard` and `benchmarkRunModel` are superseded by the S1 sweep-run model
- [ ] Delete confirmed-dead files; update remaining imports; verify 0 TS errors and all tests pass

**Audit result (2026-03-20):** None of the 8 files are dead — all are actively load-bearing. The S1 scenario system coexists with (rather than replaces) the old benchmark layer:

- `benchmarkRunModel.ts` defines `BenchmarkRunSpec`/`BenchmarkRunResult` types shared by *both* old reference families and new scenario sweeps (the store synthesizes virtual families via `activeScenarioAsFamily`).
- `ReferenceExecutionCard.svelte` is still the only UI for traditional benchmark family selection (rendered in App.svelte when `activeReferenceFamily` is set).
- `benchmarkDisclosure.ts` provides parameter snapshots/variant summaries consumed by `ReferenceExecutionCard`.
- `benchmarkCases.ts` exports benchmark family definitions and types imported by 10+ files.
- `caseCatalog.ts` re-exports from benchmarkCases + caseLibrary + presetCases; provides `CaseMode`/`ToggleState` types used across UI and stores.
- `caseLibrary.ts`, `presetCases.ts`, `phase2PresetContract.ts` all imported by caseCatalog, stores, or UI components.

**To unblock deletion**, the following migration is needed:
1. Move shared types (`BenchmarkRunSpec`, `BenchmarkRunResult`, etc.) into a new `referenceRunModel.ts` (or into `scenarios.ts`)
2. Merge `ReferenceExecutionCard` functionality into `ScenarioPicker` or a unified sensitivity-selection component
3. Migrate remaining preset cases and benchmark families into `scenarios.ts` with equivalent sensitivity dimensions
4. Inline or relocate `caseCatalog.ts` type exports (`CaseMode`, `ToggleState`, `Dimension`, etc.) into appropriate modules

### 1D. Chart & Output Polish

- [x] Scenario sweep runtime controls keep `steps` tied to the run-controls input while allowing per-variant `Δt` defaults unless the timestep field is explicitly edited.
- [x] **Color stability when sweep results arrive out of order** — `orderResults()` now sorts by `previewVariantParams` declaration index; pending preview cases also use declaration-order color indices.
- [ ] **Single-variant preview uses neutral reference color** — inconsistent with multi-variant behavior. Low priority.
- [ ] **Sweep panel has no pending overlays** — during mid-sweep, `buildSweepPanel` only shows completed results. Low priority.
- [x] **`previewBaseParams` coupling is fragile** — removed redundant `!previewVariantParams?.length` guard from App.svelte; model builder already handles variant→base precedence internally.
- [ ] **Tests missing for `previewCases` and depletion per-variant** — add coverage for pure-preview mode, mid-sweep mode, depletion with `analyticalPerVariant=true`, and `colorIndex` offset correctness.

### 1E. Documentation Refresh

- [ ] Update BENCHMARK_MODE_GUIDE.md — references pre-S1 `ModePanel.svelte` and 4-layer navigation
- [ ] Resolve SwProfileChart status — either restore the card in `App.svelte` or remove the stale component and doc references
- [ ] Document capillary pressure cap (20 × P_entry) in user-facing physics notes, not just code comments

---

## Phase 2 — Custom Mode Redesign & UX

Goal: make custom mode a deliberate power-user feature; improve input density.

### 2A. Custom Mode Redesign

Current custom mode is a catch-all that dumps 50+ raw parameter inputs with no context, no grouping intelligence, and no relationship to the predefined scenarios. It reads as legacy, not intentional.

**Focus: fully custom mode ground-up redesign.**

- [x] **Grouped parameter sections** — all sections redesigned with dense `<table>` layouts: Geometry (3×4 cells/size/total grid), Reservoir (initial conditions table + fluid PVT table + inline perm), Wells (compact 2-row table), Rel Perm (endpoints table + inline capillary + side-by-side SVG curves), Timestep (single-row table), Gas (combined rel perm + PVT in one table), Analytical (inline controls). FilterCard dimension toggles removed from custom mode panel.
- [x] **Preset starting points** — rock-type quick-pick chips (Sandstone, Carbonate, Shale/Tight, Heavy Oil) in custom mode header. Each applies domain-appropriate defaults for porosity, permeability, viscosity, saturation endpoints, capillary pressure. Defined in `reservoirPresets.ts`.
- [x] **Validation guidance** — proactive advisory warnings for low permeability (<0.1 mD), high mobility ratio (>50), large grid (>50k cells), very small timestep (<0.01 d). Wired through existing `ValidationWarning` system and `WarningPolicyPanel`.

**Postponed (revisit after custom mode lands):**

- **Clone-and-edit flow** — cloning scenario params into custom mode may be confusing. Revisit after per-scenario customisation is designed.
- **Per-scenario customisation** — allow parameter overrides *within* a predefined scenario without switching to full custom mode. Depends on the grouped layout being stable first.
- **Save/load custom configurations** — persist named custom scenarios in localStorage; export/import as JSON. Not needed initially.

### 2B. Compact Input Layout

- [x] Reduce default section padding and vertical spacing — all sections use `px-2.5 py-2` instead of `p-3`+`space-y-2`; panel wrapper uses `space-y-1 px-1 py-1.5`
- [x] Convert overly tall input groups into compact dense tables — replaced card grids, stacked label+input pairs with inline `<table>` rows
- [x] Tighten margins — Collapsible sections, table cells, checkbox labels all use minimal padding; summary text removed from section headers (data is in the tables)

### 2C. Theme Refresh

- [ ] Replace near-black dark / flat-white light surfaces with deliberate working themes
- [ ] Improve panel contrast, content focus, and data-first visual balance

### 2D. Unify Chart & Output Architecture

- [ ] Consolidate chart-shell header and expansion-state wiring across `RateChart` and `ReferenceComparisonChart`
- [ ] Finish shared panel/x-axis selection across both chart types
- [ ] Extract output selection view model from `App.svelte` — one typed helper returning the active run/result payload shared by charts, 3D, and analytical components

---

## Phase 3 — Gas Scenarios (Immiscible)

Goal: promote gas injection and solution gas drive from experimental to production-ready with analytical references. Still immiscible (constant PVT); no dissolved-gas tracking yet.

### 3A. Fix Physics (prerequisite — Phase 1A must land first)

All three-phase correctness fixes from Phase 1A are prerequisites.

### 3B. Gas-Oil Buckley-Leverett Analytical

- [x] Extend `fractionalFlow.ts` to support gas-oil displacement: `f_g(S_g)` using gas relative permeability (Corey) and gas/oil viscosity ratio
- [x] Welge tangent construction for gas-oil system — shock front saturation, breakthrough PVI
- [x] Wire as analytical overlay for `gas_injection` scenario
- [x] Add sensitivity dimensions to `gas_injection`: mobility ratio (μ_g/μ_o), S_gc, permeability, grid convergence
- [x] Validate: simulator gas breakthrough PVI vs analytical for favorable and adverse gas mobility

### 3C. Solution Gas Drive Scenario

- [ ] Define `gas_drive` base params: initially undersaturated oil with gas saturation above S_gc, BHP below bubble point
- [ ] Add sensitivity dimensions: initial gas saturation, oil viscosity, permeability
- [ ] Note: without Rs(P) tracking (Phase 4), solution gas drive is simulated as immiscible depletion with free gas — qualitatively useful but not quantitatively accurate for below-bubble-point behavior

### 3D. Gas Material Balance Diagnostics

- [ ] Add p/z diagnostic output panel — plot cumulative gas produced vs (P_i − P)/z for each timestep
- [ ] Straight line = depleting gas reservoir; curvature = aquifer influx or phase change
- [ ] Requires z-factor correlation (even simple c_g ≈ 1/P) — add as configurable option
- [ ] High diagnostic value for validating gas simulation physics

---

## Phase 4 — Black-Oil PVT (Volatile Oil)

Goal: upgrade from immiscible constant-PVT to full black-oil model with pressure-dependent fluid properties. This is the largest physics extension and unlocks volatile oil, solution gas drive with dissolved gas, and gas cap behavior.

### 4A. PVT Correlations

- [ ] **Rs(P) — solution gas-oil ratio**: Standing (1947) or Vazquez-Beggs (1980) correlation. Rs decreases as P drops below bubble point P_b; Rs = Rs_max above P_b.
- [ ] **Bo(P) — oil formation volume factor**: Standing or Glaso (1980). Bo increases with Rs (swelling); above P_b, Bo decreases slightly with pressure (compression).
- [ ] **Bg(P) — gas formation volume factor**: ideal gas law corrected by z-factor. Bg = (P_sc × z × T) / (P × T_sc).
- [ ] **μ_o(P) — oil viscosity**: Beggs-Robinson (1975) or Vasquez-Beggs. Viscosity decreases with dissolved gas; increases sharply below P_b as gas liberates.
- [ ] **μ_g(P) — gas viscosity**: Lee-Gonzalez-Eakin (1966) correlation. Varies with pressure and temperature.
- [ ] **z-factor**: Standing-Katz chart or Hall-Yarborough (1973) correlation for real gas compressibility.
- [ ] **PVT table structure in Rust**: `PvtTable` struct with interpolation; replace constant `c_o`, `c_g`, `mu_o`, `mu_g` with pressure-dependent lookups.
- [ ] **UI for PVT**: bubble-point pressure input; API gravity and gas specific gravity for correlation-based PVT; option to input tabular PVT directly.

### 4B. Bubble-Point Tracking & Phase Split

- [ ] Track dissolved gas per cell: `Rs_cell(P_cell)` — gas comes out of solution when local pressure drops below P_b
- [ ] Phase split logic: if `P < P_b`, compute `Rs(P)` and liberate excess gas → increase `S_g`
- [ ] Secondary gas cap formation: cells that drop below P_b develop free gas saturation even if initially `S_g = 0`
- [ ] Gas re-dissolution: if pressure rises above P_b (due to injection), gas dissolves back into oil
- [ ] Modify accumulation term in `step.rs`: use time-dependent Bo, Bg when computing compressibility and phase volumes

### 4C. Updated Pressure Equation

- [ ] Accumulation: `(V_p / dt) × [c_t(P) × S_phases + ...]` with pressure-dependent compressibility
- [ ] Transmissibility: mobility uses pressure-dependent viscosity from PVT lookup
- [ ] Well model: PI uses local PVT properties; GOR (gas-oil ratio) at producer from local Rs and free gas

### 4D. Volatile Oil Analytical References

- [ ] **Hyperbolic decline (Arps)** — extend `dep_decline` with Arps b-parameter (0–1): `q(t) = q_i / (1 + b × D_i × t)^(1/b)`. Current exponential (b=0) is a special case.
- [ ] **Fetkovich type-curve matching** — dimensionless decline rate and time; overlay published type curves for volatile-oil decline characterization.
- [ ] **Material-balance equation** — Havlena-Odeh (1963) formulation: `N × (E_o + m × E_g + E_fw) = F` where F = cumulative withdrawal, E terms = expansion indices. Enables OOIP estimation and drive-mechanism identification.
- [ ] **Drive-mechanism indicator** — from material balance: water drive index, gas-cap drive index, solution-gas drive index, compaction drive index. Show as stacked bar or pie diagnostic.

### 4E. Gas Cap Scenarios

- [ ] **Primary gas cap** — initial free gas zone above oil column. Configure via gas-oil contact (GOC) depth and initial gas saturation profile. Analytical: Schilthuis (1936) material balance with gas cap ratio `m`.
- [ ] **Gas cap expansion** — as oil zone depletes, gas cap expands downward. Analytical: track gas-cap movement via material balance; compare vs simulation gas front position.
- [ ] **Secondary gas cap** — forms when undersaturated oil drops below bubble point. No classical analytical solution; compare simulation against material-balance-predicted cumulative gas liberation.
- [ ] **Gas cap blowdown** — producing from gas cap after oil zone depleted. Simple volumetric depletion with p/z analysis.

### 4F. Validation

- [ ] **SPE comparative solution** benchmarks (SPE1, SPE3) for black-oil validation
- [ ] Grid-convergence study for volatile-oil depletion: verify Bo, Rs, Sg profiles converge with refinement
- [ ] Material-balance closure: cumulative production should match OOIP × recovery factor from analytical MB

---

## Phase 5 — Advanced Analytics & Sweep Methods

Goal: upgrade analytical reference methods beyond first-order approximations.

### 5A. Stiles Method (Recommended Next Sweep Upgrade)

- [ ] Implement Stiles (1949) layer-by-layer BL integration in `sweepEfficiency.ts` (~100 lines)
- [ ] Eliminates the local-PVI approximation (largest current sweep error)
- [ ] Sorts layers by permeability; applies independent BL displacement to each; sums oil production
- [ ] Exactly satisfies material balance by construction
- [ ] Keep current local-PVI method as fast baseline / teaching mode

### 5B. Warren-Root Vertical Sweep Upgrade

- [ ] Add Kv/Kh-aware blending between DP (zero cross-flow) and perfect communication (E_V → 1)
- [ ] Simple approach: `E_V_adjusted = E_V_DP × (1 − f(Kv/Kh)) + 1.0 × f(Kv/Kh)` where f is a monotonic blend function
- [ ] Resolves known mismatch where simulator (nonzero Kv) always exceeds DP analytical for vertical sweep

### 5C. Multi-Method Comparison Framework

- [ ] Selectable analytical sweep methods per scenario: local-PVI, Stiles, stream-tube
- [ ] Side-by-side comparison views: show how method choice changes RF, breakthrough timing, sweep penalty
- [ ] Compact flow-unit abstraction for representing random areal heterogeneity analytically

### 5D. Extended Pattern Correlations

- [ ] Craig tables for other well patterns: nine-spot, seven-spot, inverted five-spot, line drive
- [ ] User-selectable pattern geometry in sweep scenarios
- [ ] Line-drive: E_A = 1 in displacement direction, use BL directly

---

## Phase 6 — Multi-Case Inspection & Data I/O

Goal: make sensitivity studies inspectable beyond charts; enable data portability.

### 6A. Multi-Case 3D Inspection

- [ ] Case selection/switching for the 3D view across sensitivity runs
- [ ] Comparison awareness in saturation profile and compact summary cards
- [ ] Synchronized selected case across summary, chart, and 3D inspection

### 6B. Summary Statistics Panel

- [ ] OOIP, pore volume, recovery factor, average pressure/saturation, water cut, VRR
- [ ] Visible for both single runs and sensitivity sweeps
- [ ] Cross-case comparison table for sweep results

### 6C. Data Export/Import

- [ ] Structured scenario export/import (JSON)
- [ ] CSV/JSON export of simulation results and benchmark summaries
- [ ] Report export with plots and key metrics

### 6D. Cross-Section Viewer

- [ ] i/j/k slice viewer for property inspection in the 3D view
- [ ] Sw profile plot evolution and tighter 3D companion integration

---

## Phase 7 — Extended Physics & Well Models

Goal: advanced reservoir modeling features.

- [ ] Multi-well patterns (5-spot, line-drive, custom placements)
- [ ] Well schedule support (rate changes, shut-ins, re-completions over time)
- [ ] Aquifer boundary conditions (Carter-Tracy or Fetkovich aquifer model)
- [ ] Per-cell or per-layer porosity variation
- [ ] Per-cell initial water saturation / transition-zone initialization (J-function or specified)
- [ ] Non-uniform cell sizes and local grid refinement
- [ ] Horizontal or deviated well model with generalized Peaceman PI

---

## Backlog (Low Priority / Nice-to-Have)

- [ ] Benchmark trend tracking across CI runs
- [ ] Multi-chart synchronized zoom/pan
- [ ] Responsive/mobile chart and 3D layout improvements
- [ ] Phase relative permeability / capillary curve visualization (interactive kr/Pc editor)
- [ ] Undo/redo for parameter changes
- [ ] Per-cell capillary pressure variation and capillary hysteresis
- [ ] Leverett J-Function capillary scaling
- [ ] Grid-convergence study preset family
- [ ] A/B run comparison overlays
- [ ] Relative error (%) diagnostic curves
- [ ] Uncertainty and sensitivity batch runner beyond curated benchmark sensitivities
- [ ] Benchmark acceptance policy refresh — add tighter validation tier (~5%) alongside current 25–30% regression guards
- [ ] **Defer geometry source-of-truth redesign** — keep the current explicit `(nx, ny, nz) + (cellDx, cellDy, cellDz)` model for now. Revisit much later, only if custom-mode redesign or future non-uniform-grid work justifies the extra resolver/helper complexity.

---

## Known Constraints

- Three.js pinned at 0.183.2 — do not upgrade casually
- WASM requires `wasm32-unknown-unknown` target
- Worker ↔ UI communication: structured cloning only (no functions or class instances)
- Stop latency = one WASM step duration; for large grids (>30×30×20), single step may take >500 ms. Options: SharedArrayBuffer+Atomics, worker termination+recreate, or Rust-side callback hook.

---

## Completed

- **Mapped analytical overlays by run axis** (2026-03-20): Shared analytical waterflood/depletion overlays now remap per completed run on simulation-derived axes, preview/pending analytical curves stay hidden until mapping data exists, and the comparison chart shows an x-axis advisory when analytical overlays depend on run histories.
- **Live convergence warning updates** (2026-03-20): Convergence notices now appear on first hit, persist while later steps recover, update live with affected reference runs/counts, and still reset cleanly for a fresh non-reference run.
- **3D ternary blend + gas defaulting** (2026-03-20): Added a separate ternary saturation color-blend mode to `ThreeDView`, switched the ternary mixing from straight RGB to perceptual OKLab blending, fixed legend redraw immediately when switching both into and out of ternary mode, aligned gas saturation to the oil-style hue ramp with a `Swc`-anchored legend range, brightened the phase anchors, compacted the triangle legend, removed its border/padding/labels, and the app now defaults the 3D selector to gas saturation for gas-injection contexts.
- **3D gas saturation + waterflood default** (2026-03-20): `ThreeDView` now supports gas saturation as a selectable scalar, and the app auto-selects water saturation when the active 3D context switches into waterflood or sweep runs.
- **3D output provenance label** (2026-03-20): `ThreeDView` now renders the passed `sourceLabel`, removing the Svelte unused-export warning and keeping output provenance visible in the Spatial View header.
- **B1–B10** (2026-03-07): Benchmark modernization — family registry, explicit reference policy, sensitivity sweeps, benchmark-specific chart defaults, benchmark docs.
- **F1–F3** (2026-03): Unified Inputs/Run/Outputs shell; family-first navigation; case library; reference execution card; warning policy unified; case disclosure cards; compact Run Set selector; master-detail Results layout; IBM Plex Sans/Mono typography; semantic utility classes.
- **Store refactor** (2026-03-17): Converted `createSimulationStore()` to Svelte 5 class with `$state` fields. Fixed silent bug: 13 three-phase parameters declared as `$state` but never exposed in `parameterState` accessor.
- **Simplification Refactor Steps 1–6** (2026-03-17): `scenarios.ts` + `ScenarioPicker.svelte` replace `ModePanel.svelte` + 4-layer case-library navigation.
- **Run Controls UX** (2026-03-19): Stop button shows "Stopping…" immediately; `stopPending` state added.
- **Sweep Efficiency** (2026-03-19): Craig (1971) areal sweep, Dykstra-Parsons (1950) vertical sweep, volumetric product. `SweepEfficiencyChart.svelte`. Four new sweep scenarios.
- **S1 — Scenario/Sensitivity Architecture Redesign** (2026-03-19): Consolidated 18 scenarios → 8 canonical scenarios with multi-dimension sensitivity selection.
- **Analytical preview lifecycle** (2026-03-19): Multi-variant analytical preview before runs; mid-sweep color continuity; depletion per-variant analytical.
- **F10 — Simulation Sweep Efficiency** (2026-03-19): E_A_sim, E_V_sim, E_vol_sim computed from grid saturation. Analytical RF = E_vol × E_D_BL.
- **Chart Legend & Cases Selector Redesign** (2026-03-20): Dual-line indicator (analytical/simulation); sub-panel legends with "Simulation (N)" / "Analytical solution (N)".
