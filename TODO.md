# ResSim — TODO

## Current Issues

### Open Test Failures (5 as of 2026-03-17)

**Group A — App.svelte wiring gaps (4 failures) — ✅ Fixed 2026-03-17**
- Wired `scenario.cloneActiveReferenceToCustom()` via `onCloneReferenceToCustom` on ScenarioPicker
- Added `basePreset`, `navigationState`, `referenceProvenance`, `referenceSweepRunning`, `onActivateLibraryEntry` props to ScenarioPicker (types + interface)
- Imported and mounted `ReferenceExecutionCard` with `Reference Run Status` section and `activeRunManifest` derived
- `scenario.activeReferenceFamily?.key` now appears via `ReferenceExecutionCard` binding

**Group B — Catalog count drift (4 failures) — ✅ Fixed 2026-03-17**
Updated counts in `caseCatalog.test.ts`, `caseLibrary.test.ts`, `benchmarkRunModel.test.ts` to match `2d-grid-refinement` axis addition (16 variants, 9 run-specs, 4 axes).

**Group C — UI copy and component gaps (5 failures across `terminologyCopy.test.ts`, `modePanelFlows.test.ts`, `appThemeTypography.test.ts`, `outputTerminology.test.ts`)**
Tests describe copy strings and CSS classes not yet present in the target components. Deferred until Step 7 cleanup when ModePanel is replaced.
- [ ] Add `ui-panel-kicker` CSS class to `App.svelte` shell or `ScenarioPicker`
- [ ] Add `Library Context` and `Case Disclosure` copy to ScenarioPicker (replacing ModePanel)
- [ ] Add `Run {steps} Step` / `Advance 1 Step` / `Stop Run` to `RunControls.svelte`
- [ ] Add `Reference Guidance` / `Library sensitivity run set` / `Reference review run` to ScenarioPicker (replacing ModePanel)
- [ ] Add `Depletion Reference Solution` / `Waterflood Reference Solution` to outputs copy in `App.svelte`
- [ ] Add `ui-panel-kicker` to the main app shell header (`appThemeTypography.test.ts`)

### Store Code Quality (from 2026-03-17 refactor)

- [ ] **FRAGILE: `applyCaseParams` `||` fallback** — `|| 400` pattern silently ignores zero values for BHP/rate params. Replace with `Number.isFinite(v) ? v : default` pattern.
- [ ] **SMELL: `activeReferenceFamily` dead alias** — `get activeReferenceFamily()` and `activeReferenceBenchmarkFamily` return the same value. Consolidate to one name; audit callers.
- [ ] **OPTIMIZATION: `simWorker` and `playTimer` are `$state` unnecessarily** — neither is read in a reactive expression. Change to plain class fields.
- [ ] **SMELL: Redundant casts in `buildModelResetKey()`** — `Number(this.nx)` on already-typed `$state` fields. Remove.

---

## Active Work

### Simplification Refactor (see REFACTOR.md)

Goal: Replace 4-layer case-library navigation with `pick scenario → optionally pick sensitivity → run`.
Status: Steps 1–6 done, App.svelte wiring complete. **Step 7 (file deletion) is the last blocker, gated on Group C copy fixes.**

- ✅ **Step 4** (2026-03-17) — `buildScenarioNavigationState` removed from store (inlined via `resolveProductFamily` / `resolveScenarioSource` / `buildScenarioEditabilityPolicy`); `evaluateAnalyticalStatus` + analytical status types migrated to `warningPolicy.ts`; `phase2PresetContract.ts` re-exports for backward compat.
- [ ] **Step 7** — Delete old files once Group A App.svelte wiring (above) is complete and confirmed:
  - `src/lib/ui/modes/ModePanel.svelte`
  - `src/lib/ui/cards/ReferenceExecutionCard.svelte`
  - `src/lib/stores/phase2PresetContract.ts`
  - `src/lib/catalog/presetCases.ts`
  - `src/lib/catalog/benchmarkCases.ts`
  - `src/lib/catalog/caseCatalog.ts`
  - `src/lib/catalog/caseLibrary.ts`
  - `src/lib/benchmarkDisclosure.ts`
  - `src/lib/benchmarkRunModel.ts`
  - Note: `modePanelTypes.ts` also imports from `phase2PresetContract.ts` — audit before delete.

### F4 — Unify Chart and Output Architecture

Goal: One consistent interaction model for x-axis selection, panel expansion, legends, and output summaries across live runs and reference comparisons.

Progress so far:
- Chart legends group by metric key instead of per-case curve
- Reference-comparison charts build curves only for the focused comparison set
- Shared x-axis/log-scale helpers and panel curve selection extracted
- Compact summary cards rendered above panels from one shared output-summary contract

Remaining:
- [ ] Consolidate remaining chart-shell header and expansion-state wiring
- [ ] Finish shared panel/x-axis selection across both chart types
- [ ] Resolve any remaining Results card verbosity (F3 copy cleanup)

Acceptance:
- Chart behavior feels consistent regardless of run type
- Future output features do not require parallel implementation in both chart shells

Primary files: `src/lib/charts/RateChart.svelte`, `src/lib/charts/ReferenceComparisonChart.svelte`, `src/lib/charts/referenceComparisonModel.ts`

---

## Product Roadmap

### F5 — Multi-Case Comparison Beyond Charts

- [ ] Case selection/switching for the 3D view across sensitivity runs
- [ ] Comparison awareness in saturation profile and compact summary cards
- [ ] Synchronized selected case across summary, chart, and 3D inspection

Acceptance: sensitivity studies can be inspected spatially, not only in charts

---

### F6 — Compact Input Layout

- [ ] Reduce default section padding and vertical spacing
- [ ] Convert overly tall input groups into compact flowing cards where possible
- [ ] Keep tables only where the user is genuinely working in a tabular model
- [ ] Tighten margins and whitespace without making the UI cramped

Acceptance: common scenario editing takes materially less scrolling on desktop; dense scientific inputs remain legible

---

### F7 — Redesign Themes

- [ ] Replace near-black dark / flat-white light surfaces with deliberate working themes
- [ ] Remove or significantly soften the reservoir-layer page background treatment
- [ ] Improve panel contrast, content focus, and data-first visual balance

Acceptance: both themes feel designed for sustained technical use; decorative background no longer competes with data surfaces

---

### F8 — Scenario Builder Boundaries

- [ ] Explicitly define what belongs in Scenario Builder; do not use it as a silent fallback
- [ ] If a user is redirected there, explain why
- [ ] Reduce the number of cases that need redirection at all
- [ ] Make mode split and facet constraints readable from the UI itself

Acceptance: `Scenario Builder` reads as intentional exploratory modeling, not a catch-all bucket

---

### F9 — Refresh Docs After UI Pass

- [ ] Update README, benchmark guide, docs index, and remaining docs after F1-F8 land
- [ ] Ensure docs describe the final workflow and terminology, not transitional states

---

## Deferred / Later

### Physics Extensions

- [ ] Well schedule support
- [ ] Aquifer boundary conditions
- [ ] Per-cell or per-layer porosity variation
- [ ] Per-cell initial water saturation / transition-zone initialization
- [ ] Additional published benchmark families beyond Buckley-Leverett and depletion

### Benchmark and Comparison Tooling

- [ ] Grid-convergence study preset family
- [ ] A/B run comparison overlays
- [ ] Relative error (%) diagnostic curves
- [ ] Uncertainty and sensitivity batch runner beyond curated benchmark sensitivities

### Visualization and Charting

- [ ] Sw profile plot evolution and tighter 3D companion integration
- [ ] Cross-section / slice viewer for i/j/k inspection in the 3D view
- [ ] Summary statistics panel (OOIP, pore volume, RF, average pressure/saturation, water cut, VRR)

### Scenario and Reporting

- [ ] Structured scenario export/import
- [ ] CSV/JSON export of results and benchmark summaries

### Wells and Advanced Reservoir Modeling

- [ ] Multi-well patterns (5-spot, line-drive, custom placements)
- [ ] Non-uniform cell sizes and local grid refinement

### Analytical and Diagnostic Expansion

- [ ] Areal sweep efficiency charting
- [ ] Depletion analytical calibration against additional published references

### Nice To Have Only

- [ ] Benchmark trend tracking across CI runs
- [ ] Comparative visualization: side-by-side scenarios or delta views
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
- **F1** (2026-03): Unified `Inputs / Run / Outputs` shell; family-first navigation; case library; reference execution card in Run region; comparison moved to Outputs; legacy benchmark-mode plumbing removed.
- **F2** (2026-03): Warning policy unified (`Action Required`, `Reliability Cautions`, `Reference Limits`, `Run Notes`); vocabulary normalized to `Reference Solution`, `Reference Guidance`, `Run Set` throughout UI and docs.
- **F3** (2026-03): Case disclosure cards; compact `Run Set` selector with variant deltas; master-detail Results layout; compact run table; shared IBM Plex Sans/Mono typography baseline; semantic utility classes (`ui-panel-kicker`, `ui-section-kicker`, `ui-chip`, etc.).
- **Store refactor** (2026-03-17): Converted `createSimulationStore()` from function-based getter/setter boilerplate (~140 lines eliminated) to a Svelte 5 class with `$state` fields. Fixed silent bug: 13 three-phase parameters were declared as `$state` but never exposed in the `parameterState` accessor object.
- **Group B catalog fixes** (2026-03-17): Updated test counts for `2d-grid-refinement` axis addition (16 variants, 9 run-specs, 4 axes).
- **REFACTOR Step 4** (2026-03-17): `evaluateAnalyticalStatus` + analytical status types moved to `warningPolicy.ts`; `buildScenarioNavigationState` removed from store (inlined); backward-compat re-exports added to `phase2PresetContract.ts`.
- **Group A App.svelte wiring** (2026-03-17): `cloneActiveReferenceToCustom`, `basePreset`, `navigationState`, `referenceProvenance`, `onActivateLibraryEntry` wired through ScenarioPicker; `ReferenceExecutionCard` mounted in Run section with `activeRunManifest`. 10/10 `appStoreDomainWiring.test.ts` tests pass.
