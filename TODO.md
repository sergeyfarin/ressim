# ResSim — Roadmap

Phased plan to evolve ResSim from a validated 2-phase waterflood teaching tool into a full black-oil simulator with gas, volatile-oil, and compositional-aware analytics.

---

## Phase 1 — Consolidate & Fix (current foundations)

Goal: clean up tech debt, fix known correctness issues, finish incomplete work.

### 1A. Three-Phase Correctness Fixes

Confirmed bugs blocking gas scenarios from leaving "experimental" status.

- [ ] **Gas-oil capillary pressure direction** — `capillary.rs`: P_cog currently *decreases* with increasing S_g. Physical requirement: gas is non-wetting, P_cog must increase with S_g. Fix: parameterise on S_o_eff using `S_org`.
- [ ] **Stone II missing `S_org` parameter** — `relperm.rs`: `k_ro_gas` uses `s_gr` as terminal oil saturation in a gas flood; should use a distinct `s_org` (residual oil to gas, typically > `s_or`). Add `s_org` and wire through `k_ro_gas`, capillary pressure, and UI.
- [ ] **3-phase material-balance diagnostic** — `step.rs`: `actual_change_m3` accumulates only ΔS_w × Vp. Add parallel accumulators for gas and oil so all three phases are covered.

### 1B. Analytical Contract Gaps

- [ ] **Dietz well-location sensitivity** — `depletionAnalytical.ts` infers shape factor from aspect ratio only; does not consume `producerI`/`producerJ`. The `dep_pss` shape-factor dimension marks `affectsAnalytical: false` — technically correct (analytical doesn't change) but conceptually wrong (it *should* change). Fix: pass producer position or explicit C_A into the depletion analytical adapter; update `affectsAnalytical: true`; add test proving center (C_A ≈ 30.88) and corner (C_A ≈ 0.56) analytical curves diverge.
- [ ] **Analytical adapter coverage tests** — add contract tests that every sensitivity dimension marked `affectsAnalytical: true` actually changes at least one input consumed by the analytical builder for that scenario class. Prevents future Dietz-style gaps.
- [ ] **Analytical method disclosure upkeep** — keep scenario-level analytical method summary/reference metadata aligned with the actual overlay path shown in the UI whenever analytical routing changes.

### 1C. Legacy Cleanup (Phase 1 Step 7 Remainder)

Eight legacy catalog/benchmark files still have active production dependencies. The blocker is that `ReferenceExecutionCard`, `benchmarkRunModel`, and the chart layer still import from old catalog files.

- [ ] Audit whether `ReferenceExecutionCard` and `benchmarkRunModel` are superseded by the S1 sweep-run model
- [ ] Delete confirmed-dead files; update remaining imports; verify 0 TS errors and all tests pass

Files pending deletion: `ReferenceExecutionCard.svelte`, `benchmarkCases.ts`, `benchmarkRunModel.ts`, `benchmarkDisclosure.ts`, `caseCatalog.ts`, `caseLibrary.ts`, `presetCases.ts`, `phase2PresetContract.ts`

### 1D. Chart & Output Polish

- [ ] **Color stability when sweep results arrive out of order** — `orderResults()` sorts by variant presence then insertion order. Fix: sort by variant declaration index from `previewVariantParams` order, not arrival order.
- [ ] **Single-variant preview uses neutral reference color** — inconsistent with multi-variant behavior. Low priority.
- [ ] **Sweep panel has no pending overlays** — during mid-sweep, `buildSweepPanel` only shows completed results. Low priority.
- [ ] **`previewBaseParams` coupling is fragile** — add defensive guard against preview/result condition divergence in `App.svelte`.
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

**Redesign direction:**

- [ ] **Clone-and-edit flow** — entering custom mode should clone the currently active scenario's params as a starting point, not reset to defaults. Show provenance ("Based on: 1D Waterflood — Mobility Ratio").
- [ ] **Per-scenario customisation** — allow parameter overrides *within* a predefined scenario without switching to full custom mode. The scenario stays active; the user sees which params they've changed and can reset individual overrides. This replaces the need to "go custom" for small parameter tweaks.
- [ ] **Grouped parameter sections** — replace flat 50-field form with domain-aware groups: Rock Properties, Fluid PVT, Wells, Grid & Timestep, Relative Permeability (SCAL), Gas/3-Phase. Each group collapsible; show only groups relevant to the active scenario class.
- [ ] **Preset starting points** — quick-pick buttons for common reservoir types (sandstone, shale, carbonate, tight gas) that set reasonable default ranges for porosity, permeability, fluid properties.
- [ ] **Save/load custom configurations** — persist named custom scenarios in localStorage; allow export/import as JSON.
- [ ] **Validation guidance** — proactive parameter range warnings (e.g. "permeability < 0.1 mD: convergence may be slow") instead of only post-run validation errors.

### 2B. Compact Input Layout

- [ ] Reduce default section padding and vertical spacing
- [ ] Convert overly tall input groups into compact flowing cards
- [ ] Tighten margins without making the UI cramped; common scenario editing should take materially less scrolling

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

- [ ] Extend `fractionalFlow.ts` to support gas-oil displacement: `f_g(S_g)` using gas relative permeability (Corey) and gas/oil viscosity ratio
- [ ] Welge tangent construction for gas-oil system — shock front saturation, breakthrough PVI
- [ ] Wire as analytical overlay for `gas_injection` scenario
- [ ] Add sensitivity dimensions to `gas_injection`: mobility ratio (μ_g/μ_o), S_gc, permeability, grid convergence
- [ ] Validate: simulator gas breakthrough PVI vs analytical for favorable and adverse gas mobility

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

---

## Known Constraints

- Three.js pinned at 0.183.2 — do not upgrade casually
- WASM requires `wasm32-unknown-unknown` target
- Worker ↔ UI communication: structured cloning only (no functions or class instances)
- Stop latency = one WASM step duration; for large grids (>30×30×20), single step may take >500 ms. Options: SharedArrayBuffer+Atomics, worker termination+recreate, or Rust-side callback hook.

---

## Completed

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
