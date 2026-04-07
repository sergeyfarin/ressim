# ResSim Frontend Refactor — Analysis & Step-by-Step Plan

> Last updated: 2026-04-07. Phases 0–4 complete.

---

## 1. Goals

The refactor has seven concrete objectives:

1. **App.svelte is lean** — orchestrates, does not process data.
2. **Maximum processing in Rust** — simulation output arrives at Svelte ready to visualize.
3. **Analytical/reference curves in separate files** — pure functions, not Svelte components.
4. **Chart component is completely agnostic** — plots any panels; knows nothing about scenarios.
   Consistent styling contract: solid = simulation, dashed = analytical, dotted = published reference.
5. **Axis conversions (PVI → time, etc.) in dedicated files** — not inlined in chart builders.
6. **Scenarios are the single source of truth** — define simulation params, analytical function,
   published reference series, and which chart panels to display. No hidden fallbacks.
7. **Common chart panels defined once** — reusable panel definitions that carry behaviour and look.

---

## 2. Current State After WIP Changes (2026-03-30)

### 2.1 What the WIP changes deliver

#### Rust / WASM (`frontend.rs` + `pkg/`)

| New API | What it does | Impact |
|---------|-------------|--------|
| `getGridState()` | Returns all four grid arrays (pressure, Sw, So, Sg) as a single JS object using `Float64Array::view()` — **zero-copy** while the call is live; the worker structured-clones into UI-owned memory immediately after | Replaces 4 × `Vec::clone()` per step; eliminates ~4 × (nx·ny·nz × 8 bytes) of redundant copies |
| `getRateHistorySince(startIndex)` | Returns only the tail of `rate_history` since `startIndex` | Fixes the O(n²) full-history serialization; a 500-step run now copies 1 entry per step instead of 1+2+…+500 |

#### Worker (`sim.worker.ts`)

- `lastRateHistoryLen` counter tracks how many history points have already been sent.
- Calls `getRateHistorySince(lastRateHistoryLen)` each step; appends delta to UI.
- Counter correctly reset on `configureSimulator()`, `dispose`, and state-reload paths.
- `getGridState()` replaces the four separate getter calls and the `typeof getSatGas === 'function'` feature-detect.

#### Store (`simulationStore.svelte.ts`)

- Handles both `rateHistory` (full, used for state-reload) and `rateHistoryDelta` (incremental, used during a run).

#### App.svelte

| Before | After |
|--------|-------|
| 30 individual `outputProfile*` `$derived` declarations | Single `selectedOutputProfile: OutputSelectionProfile` object |
| 8 individual `output3D*` `$derived` declarations | Single `selectedOutput3D: Output3DSelection` object |
| Hidden `<FractionalFlow>` component firing DOM events | `calculateAnalyticalProduction()` called as pure function inside `liveAnalyticalOutput` derived |
| Hidden `<DepletionAnalytical>` component firing DOM events | `calculateDepletionAnalyticalProduction()` called as pure function inside `liveAnalyticalOutput` derived |
| `runtime.analyticalProductionData` / `runtime.analyticalMeta` updated via event callbacks | `liveAnalyticalOutput.production` / `.meta` read directly as reactive derived |

#### TODO.md

Marked done:
- `[x]` Grid-state bundling and incremental rate-history extraction.
- `[x]` Typed output-selection view model extracted from App.svelte.

New items added:
- `[ ]` Move sweep-efficiency reporting into Rust step reports.
- `[ ]` Restore TypeScript typecheck health (failures in `buildCreatePayload.ts`, `sim.worker.ts`, test and script files).

---

### 2.2 What remains a problem

#### App.svelte (still ~820 lines)

| Issue | Severity | Detail |
|-------|----------|--------|
| `liveAnalyticalOutput` branches on `params.analyticalMode` string | High | Still routes by `"waterflood"` / `"depletion"` strings instead of a scenario-owned function reference |
| `sweepEfficiencySimSeries` is still O(n²) `$derived` | High | Iterates all history × all cells on every render; not incremental |
| `analyticalMode` string still used for routing in 3+ more places | Medium | `outputProfileScenarioMode`, sweep logic, `RateChart` mode passing |
| Prop drilling still deep | Medium | 20+ props to RateChart, 15+ to ThreeDViewComponent |
| Analytical calculation inlined with parameter spreading | Medium | `liveAnalyticalOutput` calls analytical fn with ~20 individually named params — parameters should be assembled by the scenario definition |

#### Chart layer

| Issue | Severity | Detail |
|-------|----------|--------|
| `referenceComparisonModel.ts` — 3,285 lines | Critical | Imports all analytical functions, remaps PVI→time axes, manages colors, assembles all panels. Does everything. |
| `RateChart.svelte` — 1,568 lines | High | Hard-codes string matching on `activeMode` / `activeCase` for panel expansion defaults |
| Analytical curves computed at chart-model-build time | High | BL / depletion solutions recalculated each time comparison model runs; PVI→time remapping not cached |
| Styling constants duplicated | Medium | `borderDash: [7, 4]` / `[4, 4]` / `[2, 4]` repeated across multiple files |
| Legend section strings duplicated | Low | `'Simulation (solid lines):'` repeated 4+ times |

#### Scenario → chart coupling

| Issue | Severity | Detail |
|-------|----------|--------|
| No single file owns "what panels this scenario shows" | High | Computed across `chartLayouts.ts` + `scenarios.ts` + `ANALYTICAL_OUTPUT_CONTRACTS` + `RateChart` logic |
| Scenario does not own its analytical function | High | Scenarios declare `analyticalMethod: string`; routing to the actual function lives in App.svelte and `referenceComparisonModel.ts` |
| `analyticalMode` vs `analyticalMethod` duality | Medium | Two overlapping routing keys; `analyticalMode` is a param-level field, `analyticalMethod` is a capability field — both used |

#### Store (`simulationStore.svelte.ts` — 1,995 lines)

| Issue | Severity | Detail |
|-------|----------|--------|
| God object: 110+ `$state` fields | High | Parameter state, runtime state, navigation state all entangled |
| `activeScenarioAsFamily` derived (~80 lines) | High | Builds a synthetic `BenchmarkFamily` — chart-layer concern inside the store |
| `pvtTable` derived from 20+ inputs | Medium | Regenerates full black-oil table on every parameter change |
| Worker lifecycle inside state class | Medium | `setupWorker()` mixes worker management with state |

#### TypeScript typecheck (new, urgent)

Failures in:
- `src/lib/buildCreatePayload.ts` — `controlMode: string` where `"pressure" | "rate"` expected
- `src/lib/workers/sim.worker.ts` — same control-mode narrowing issue
- `src/lib/catalog/benchmarkPresetRuntime.test.ts` — references removed fields
- `scripts/debug-spe1-grid5.ts` / `scripts/debug-spe1-gas.ts` — call simulator methods that no longer exist

---

## 3. Target Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│  SCENARIOS  src/lib/catalog/scenarios/                               │
│                                                                      │
│  Each scenario is fully self-describing:                             │
│  • params          — complete simulation inputs                      │
│  • analyticalDef   — { fn, inputsFromParams }  (function reference) │
│  • chartPanels     — ChartPanelRef[]  (what to show, where)         │
│  • publishedReferenceSeries — static benchmark overlays              │
│  No string-routing. No hidden fallbacks.                             │
└────────┬────────────────┬─────────────────────┬──────────────────────┘
         │                │                     │
         ▼                ▼                     ▼
┌──────────────┐  ┌──────────────────┐  ┌──────────────────────────┐
│  RUST/WASM   │  │  ANALYTICAL      │  │  CHART PANELS            │
│  ressim core │  │  src/lib/        │  │  src/lib/charts/         │
│              │  │  analytical/     │  │  panels.ts               │
│  Produces:   │  │                  │  │                          │
│  • grid state│  │  Pure functions: │  │  Panel catalog:          │
│    (bundled) │  │  • computeBL()   │  │  PANEL_DEFS.rates        │
│  • rate delta│  │  • computeDep()  │  │  PANEL_DEFS.recovery     │
│  • sweep pts │  │  • computeSweep()│  │  PANEL_DEFS.pressure     │
│    per step  │  │                  │  │  PANEL_DEFS.diagnostics  │
│  (✓ getGrid  │  │  Adapters:       │  │  PANEL_DEFS.gor          │
│   State done)│  │  • pviToTime()   │  │  PANEL_DEFS.sweep        │
│  (✓ getRate  │  │  • normalizeRF() │  │                          │
│   HistSince) │  │  src/lib/        │  │  Each defines:           │
└──────┬───────┘  │  analytical/     │  │  • title, yAxisLabel     │
       │          │  axisAdapters.ts │  │  • scalePreset           │
       ▼          └───────┬──────────┘  │  • allowLogToggle        │
┌──────────────┐          │             │  • legendSection text    │
│  WORKER      │          │             └──────────┬───────────────┘
│  (✓ bundled  │          │                        │
│   extraction)│          ▼                        ▼
└──────┬───────┘  ┌───────────────────────────────────────────────────┐
       │          │  CHART DATA BUILDER                               │
       ▼          │  src/lib/charts/buildChartData.ts                 │
┌──────────────┐  │                                                   │
│  STORES      │  │  buildChartDataSets(scenario, results, options)   │
│  (3 focused  │  │                                                   │
│   stores)    │  │  1. scenario.analyticalDef.fn(params)             │
│              │  │  2. axisAdapters.pviToTime(curves, rateHistory)   │
│  • params    │  │  3. apply curveStylePolicy consistently           │
│  • runtime   │  │  4. group by panel → ChartDataSet[]               │
│  • navigation│  │                                                   │
└──────┬───────┘  │  All styling in curveStylePolicy.ts (one file)   │
       │          └─────────────────────┬─────────────────────────────┘
       └────────────────────────────────▼
                               ┌────────────────────────┐
                               │  App.svelte  (LEAN)    │
                               │  < 200 lines target    │
                               │                        │
                               │  • imports 3 stores    │
                               │  • lazy-loads modules  │
                               │  • passes ChartDataSet │
                               │    to UniversalChart   │
                               │  • passes outputProfile│
                               │    to ThreeDView       │
                               │  • no data processing  │
                               └──────────┬─────────────┘
                                          │
                   ┌──────────────────────┼──────────────────────┐
                   ▼                      ▼                      ▼
          ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
          │  UniversalChart  │  │  ThreeDView       │  │  ScenarioPicker  │
          │  (agnostic)      │  │  (thin props)     │  │  (unchanged)     │
          │  • ChartDataSet[]│  │  Output3DSelection│  │                  │
          │  • no scenario   │  │  object (✓ done)  │  │                  │
          │    knowledge     │  └──────────────────┘  └──────────────────┘
          └──────────────────┘
```

### Key new types

```typescript
// Scenario owns its analytical function
type ScenarioAnalyticalDef = {
    fn: (inputs: AnalyticalInputs) => AnalyticalCurves;
    inputsFromParams: (params: ScenarioParams, rateHistory: RateHistoryPoint[]) => AnalyticalInputs;
};

// Scenario declares exactly which panels it uses (no hidden layout merging)
type ChartPanelRef = {
    panelKey: keyof typeof PANEL_DEFS;
    curveKeys: string[];           // which sim/analytical/published curves to show
    xAxisMode: XAxisMode;
    expanded?: boolean;
};

// Panel definitions live in one place
type ChartPanelDef = {
    key: string;
    title: string;
    yAxisLabel: string;
    scalePreset: ScalePreset;
    allowLogToggle: boolean;
    legendSection: string;        // e.g. 'Avg Reservoir Pressure (bar)'
};

// Builder output: ready for UniversalChart
type ChartDataSet = {
    panelKey: string;
    panelDef: ChartPanelDef;
    curves: CurveConfig[];        // styling pre-applied, data embedded
    xValues: number[];
};
```

---

## 4. Step-by-Step Plan

Phases are ordered by dependency and value. Each phase leaves the application fully working.

---

### Phase 0 — COMPLETE ✓

**Delivered in this WIP batch:**

- `getGridState()` — bundled zero-copy grid extraction in Rust.
- `getRateHistorySince()` — incremental rate history; fixes O(n²) serialization.
- Worker updated to use both new APIs with correct counter management.
- Store appends deltas correctly.
- `selectedOutputProfile` — collapses 30 `outputProfile*` derived into one typed object.
- `selectedOutput3D` — collapses 8 `output3D*` derived into one typed object.
- Hidden `FractionalFlow.svelte` and `DepletionAnalytical.svelte` components removed.
- `liveAnalyticalOutput` derived calls pure analytical functions directly.

**Remaining loose end from Phase 0:** `liveAnalyticalOutput` still branches on
`params.analyticalMode` string rather than a scenario-owned function reference.
That is addressed in Phase 2.

---

### Phase 0.5 — Fix TypeScript Typecheck ✓ COMPLETE

**Why:** Typecheck failures block confident refactoring. Introduced alongside the WIP changes.

**Files fixed:**
- [src/lib/buildCreatePayload.ts](src/lib/buildCreatePayload.ts) — narrow `controlMode` from `string` to `"pressure" | "rate"`
- [src/lib/workers/sim.worker.ts](src/lib/workers/sim.worker.ts) — same narrowing issue
- [src/lib/catalog/benchmarkPresetRuntime.test.ts](src/lib/catalog/benchmarkPresetRuntime.test.ts) — update removed field references
- [scripts/debug-spe1-grid5.ts](scripts/debug-spe1-grid5.ts) / [scripts/debug-spe1-gas.ts](scripts/debug-spe1-gas.ts) — update to current WASM API

**Verification:** `npm run typecheck` passes clean.

---

### Phase 1 — Move Sweep Metrics to Rust ✓ COMPLETE

**Goal:** Eliminate the O(n²) `sweepEfficiencySimSeries` derived in App.svelte.
Rust computes eA, eV, eVol, mobileOilRecovered per step in `record_step_report()`.

**Rust changes (done):**

- `src/lib/ressim/src/reporting.rs` — new `SweepConfig` struct (geometry, swept_threshold, initial/residual oil saturation) + `SweepMetrics` struct + optional `sweep: Option<SweepMetrics>` on `TimePointRates`. Binary threshold sweep computed in `compute_sweep_metrics()`. Called in BOTH `record_step_report()` (IMPES) AND `record_fim_step_report()` (FIM) — solver-agnostic.
- `src/lib/ressim/src/lib.rs` — added `sweep_config: Option<SweepConfig>` field.
- `src/lib/ressim/src/frontend.rs` — added `setSweepConfig(config_js: JsValue)` WASM setter.
- Rebuilt WASM pkg (wasm-bindgen 0.2.117 bundler target).

**TypeScript changes (done):**

- [src/lib/simulator-types.ts](src/lib/simulator-types.ts) — added `sweepConfig?` to `SimulatorCreatePayload`; added `sweep?: { e_a?, e_v?, e_vol, mobile_oil_recovered? }` to `RateHistoryPoint`.
- [src/lib/stores/simulationStore.svelte.ts](src/lib/stores/simulationStore.svelte.ts) — `buildCreatePayload()` now appends `sweepConfig` using scenario capabilities + threshold = `s_wc + 0.2 × movable_range`.
- [src/lib/workers/sim.worker.ts](src/lib/workers/sim.worker.ts) — calls `setSweepConfig(payload.sweepConfig)` if present; removed stale `init()` call (wasm-bindgen 0.2.117 auto-init).
- [src/lib/ressim/pkg/simulator_bg.d.ts](src/lib/ressim/pkg/simulator_bg.d.ts) — created hand-maintained ambient declarations for `simulator_bg.js` (re-exports `simulator.d.ts` + `__wbg_set_wasm`) enabling Node.js WASM bootstrap.
- [src/lib/catalog/benchmarkPresetRuntime.test.ts](src/lib/catalog/benchmarkPresetRuntime.test.ts) — updated WASM init to use Node.js manual bootstrap (`WebAssembly.instantiate` + `__wbg_set_wasm`).
- [scripts/debug-spe1-grid5.ts](scripts/debug-spe1-grid5.ts) — same Node.js WASM bootstrap update.

**What is NOT yet done (next step):**

- [src/App.svelte](src/App.svelte) — `sweepEfficiencySimSeries` `$derived` still present; should be deleted in favour of reading `.sweep` from rate history.
- [src/lib/charts/referenceComparisonModel.ts](src/lib/charts/referenceComparisonModel.ts) — sweep series should read from `rateHistory[i].sweep` instead of calling `computeSimSweepDiagnosticsForGeometry()` per history entry.

**Verification:** 188 Rust tests pass. TypeScript typecheck clean. 523/530 TypeScript tests pass (7 pre-existing failures unrelated to these changes).

---

### Phase 2 — Scenarios Own Their Analytical Function ✓ COMPLETE

**Delivered 2026-04-04:**

- `src/lib/catalog/scenarios.ts` — added `ScenarioAnalyticalPoint`, `ScenarioAnalyticalMeta`,
  `ScenarioAnalyticalOutput`, and `ScenarioAnalyticalDef` types. Added `analyticalDef?:
  ScenarioAnalyticalDef` field to `Scenario`.
- `src/lib/catalog/analyticalAdapters.ts` — new file with three pre-built defs shared
  across scenarios: `waterfloodBLDef`, `depletionDef`, `gasOilBLDef`.
- 8 of 10 scenario files populated with `analyticalDef` (gas_drive = `none`, spe1 = `digitized-reference` have no analytical function).
- [src/App.svelte](src/App.svelte) — `liveAnalyticalOutput` reduced from ~65 lines to 6:
  ```typescript
  const liveAnalyticalOutput = $derived.by((): ScenarioAnalyticalOutput => {
      if (runtime.rateHistory.length === 0) return EMPTY_ANALYTICAL_OUTPUT;
      const def = scenario.activeScenarioObject?.analyticalDef;
      if (!def) return EMPTY_ANALYTICAL_OUTPUT;
      const inputs = def.inputsFromParams(params as unknown as Record<string, unknown>, runtime.rateHistory);
      return def.fn(inputs);
  });
  ```
- `AppAnalyticalMeta` and `AppAnalyticalPoint` local type aliases replaced by index types
  derived from `ScenarioAnalyticalOutput`.

**Not done (deferred to Phase 4):**
- `referenceComparisonModel.ts` — still uses `analyticalMethod` string routing internally via
  `computeBLAnalyticalFromParams`, `computeDepletionAnalyticalFromParams` etc. The `analyticalDef`
  is now available on scenarios for Phase 4 to consume when it replaces that file entirely.
- `src/lib/analytical/index.ts` — public re-export barrel; not needed until Phase 4 consumers land.
- `src/lib/analytical/axisAdapters.ts` — axis conversion utilities; deferred to Phase 4.

**Verification:** 533 TypeScript tests pass. `npm run typecheck` clean.

---

### Phase 3 — Chart Panel Catalog & Curve Style Policy ✓ COMPLETE

**Delivered:**

- `src/lib/charts/panelDefs.ts` (124 lines) — `PANEL_DEFS` catalog; imported by both
  `RateChart.svelte` and `ReferenceComparisonChart.svelte`.
- `src/lib/charts/curveStylePolicy.ts` (87 lines) — single source for `ANALYTICAL_DASH`,
  `PUBLISHED_DASH`, `AUXILIARY_DASH`, `SWEEP_DASH_*`, `LEGEND_SECTIONS`, `ANALYTICAL_BORDER`,
  `simBorderWidth`. Imported across all chart builders.

**Not done (deferred):**
- `chartPanels: ChartPanelRef[]` on `Scenario` — deferred to Phase 8 (benchmark consolidation).
  Scenarios still use `chartLayoutKey` + `layoutConfig` through `BenchmarkFamily`.
- `chartLayouts.ts` refactor — the merge machinery remains; simplification deferred to Phase 8.

**Verification:** `npm run typecheck` clean. `referenceChartConfig.test.ts` passes.

---

### Phase 4 — `buildChartData.ts` replaces `referenceComparisonModel.ts` ✓ COMPLETE

**Delivered 2026-04-07:**

The 2,560-line `referenceComparisonModel.ts` was split into four focused files and deleted.

| File | Lines | Responsibility |
|------|-------|---------------|
| `src/lib/charts/referenceChartTypes.ts` | ~175 | Types, color palette, panel construction helpers |
| `src/lib/charts/referenceOverlayBuilders.ts` | ~180 | BL / depletion / gas-oil analytical overlays |
| `src/lib/charts/sweepPanelBuilder.ts` | ~500 | Sweep efficiency panels (sim + analytical) |
| `src/lib/charts/buildChartData.ts` | ~680 | Main orchestrator (`buildReferenceComparisonModel`) |

- `analyticalParamAdapters.ts` — `computeDepletionOnTimeAxis` added; `analyticalParamAdapters.test.ts`
  created (65 tests).
- All three consumers updated: `ReferenceComparisonChart.svelte`, `referenceComparisonModel.test.ts`,
  `benchmarkPresetRuntime.test.ts`.
- `src/lib/charts/referenceComparisonModel.ts` — **deleted** ✓

**Not done (deferred to Phase 5):**
- `buildRateChartData.ts` — the live-chart builder for `RateChart.svelte` was not created;
  `RateChart.svelte` still builds curves inline (1,419 lines).
- The `buildChartDataSets(scenario, results, options): ChartDataSet[]` signature from the plan
  was not adopted; `buildReferenceComparisonModel` kept its name for backward compat while
  `BenchmarkFamily` remains the scenario contract (deferred to Phase 8).

**Verification:** `npm run typecheck` exits 0. 627 TypeScript tests pass (6 pre-existing WASM
`LinkError` failures unrelated to these changes).

---

### Phase 5 — Extract `buildRateChartData.ts`, trim `RateChart.svelte`

**Goal:** `RateChart.svelte` (1,419 lines) stops building curves inline. A new
`buildRateChartData.ts` extracts all curve-assembly logic into a pure function,
mirroring what Phase 4 did for the reference chart.

**Starting point:**
- `RateChart.svelte` — 1,419 lines. Contains: 20-prop interface, x-axis/log-scale
  state, scenario-aware panel-default `$effect` (string-matching on `activeMode`/`activeCase`),
  inline curve builders for waterflood / depletion / gas-oil-bl / sweep, panel rendering.
- `ChartSubPanel.svelte` — 791 lines. Already a focused Chart.js wrapper (curves + series
  in, Chart.js out). Stays mostly as-is.
- `ReferenceComparisonChart.svelte` — 576 lines. Already thin (calls `buildReferenceComparisonModel`,
  renders panels). Minor trimming possible.

**Files to create:**

- `src/lib/charts/buildRateChartData.ts` — pure function that takes `rateHistory`, analytical
  curves, sweep series, and options; returns the same `ReferenceComparisonPanelMap` shape
  consumed by `RateChart.svelte`'s panel renderer:
  ```typescript
  export function buildRateChartData(input: {
      rateHistory: RateHistoryPoint[];
      analyticalProductionData: AnalyticalProductionPoint[];
      avgReservoirPressureSeries: Array<number | null>;
      avgWaterSaturationSeries: Array<number | null>;
      ooipM3: number;
      poreVolumeM3: number;
      activeMode: string;
      theme: 'dark' | 'light';
      xAxisMode: RateChartXAxisMode;
      // ... sweep inputs
  }): ReferenceComparisonPanelMap
  ```
  Imports from `referenceChartTypes.ts` (appendSeries, panel helpers) and
  `curveStylePolicy.ts` (styling constants).

**Files to modify:**

- [src/lib/charts/RateChart.svelte](src/lib/charts/RateChart.svelte) — delete inline curve builders.
  Call `buildRateChartData()`, pass result panels to `<ChartSubPanel>` instances.
  Target: under 300 lines (state management + layout only).

**Files NOT changed in this phase:**
- `ChartSubPanel.svelte` — already a good Chart.js wrapper; leave it.
- `ReferenceComparisonChart.svelte` — already delegates to `buildChartData.ts`; minor cleanup only.
- `UniversalChart.svelte` — the shared panel renderer is a desirable long-term goal but requires
  both charts to share the same panel-map contract first. Deferred to after Phase 6.

**Verification:** Live simulation charts render identically for all 10 scenarios.
`RateChart.svelte` diff shows only state management and layout remaining.

---

### Phase 6 — Split the Store (2-3 days)

**Goal:** Break 1,995-line `simulationStore.svelte.ts` into three focused stores.

**Files to create:**

- `src/lib/stores/parameterStore.svelte.ts` — all 110 simulation input parameters, PVT table derivation, validation.
- `src/lib/stores/runtimeStore.svelte.ts` — worker lifecycle, simulation output, history, playback, sweep history.
- `src/lib/stores/navigationStore.svelte.ts` — active scenario key, sensitivity, variants, mode.
  Remove `activeScenarioAsFamily` (chart-layer concern; eliminated by Phase 4).

**Migration strategy:**

Use a transitional barrel `src/lib/stores/simulationStore.svelte.ts` that re-exports from the
three new files during the migration window, then delete it once all consumers are updated.

**Files to delete (after migration):**

- `src/lib/stores/simulationStore.svelte.ts` — replaced by the three above.

**Verification:** All parameter editing, scenario selection, and run controls work. No
`activeScenarioAsFamily` usage remains.

---

### Phase 7 — Lean App.svelte (1-2 days)

By Phase 6, App.svelte's remaining logic should already be small. This phase finishes it.

**Target App.svelte (< 200 lines):**
- Import three stores.
- Lazy-load chart and 3D modules in `onMount`.
- Wire run callbacks (`handleRun`, `handleStop`, `handleApplyHistoryIndex`).
- Render `<ScenarioPicker>`, `<RunControls>`, `<UniversalChart datasets={...}>`,
  `<ThreeDView {...selectedOutput3D}>`.
- Theme management (3 effects).
- No analytical logic. No data processing. No `$derived` with business logic.

**Verification:** App.svelte diff shows only orchestration code.

---

### Phase 8 — Benchmark Layer Consolidation (3-4 days)

Addresses ROADMAP Priority 3.1.

**Goal:** Scenarios drive reference runs directly. Legacy `benchmarkCases.ts`, `caseCatalog.ts`,
and related adapters are removed. The synthetic `BenchmarkFamily` object built in the store
disappears.

**Files to delete (after migration):**

- `src/lib/catalog/benchmarkCases.ts`
- `src/lib/catalog/caseCatalog.ts` (or equivalent legacy adapters)

**Files to modify:**

- [src/lib/catalog/scenarios.ts](src/lib/catalog/scenarios.ts) — `Scenario` type gains reference-run metadata
  directly (what `BenchmarkFamily` currently carries).
- [src/lib/stores/navigationStore.svelte.ts](src/lib/stores/navigationStore.svelte.ts) — remove `activeScenarioAsFamily`
  entirely.
- SPE1 scenario — verify all panels, published reference overlays, chart styling.
- Add missing regression tests: comparison-model preview, per-variant depletion analytics,
  color-index stability (from ROADMAP 1.3).

**Verification:** All benchmark runs complete; all comparison charts correct; no legacy files remain.

---

## 5. Execution Order & Dependencies

```
Phase 0.5  ──── TypeScript typecheck fix ✓ DONE
    │
    ├── Phase 1  ──── Sweep metrics in Rust ✓ DONE
    │
    └── Phase 2  ──── Scenarios own analytical fn ✓ DONE
            │
            └── Phase 3  ──── Panel catalog + style policy ✓ DONE
                    │
                    └── Phase 4  ──── buildChartData replaces referenceComparisonModel ✓ DONE
                            │
                            └── Phase 5  ──── buildRateChartData + trim RateChart.svelte  ← NOW
                                    │
                                    └── Phase 6  ──── Split store
                                            │
                                            └── Phase 7  ──── Lean App.svelte
                                                    │
                                                    └── Phase 8  ──── Benchmark consolidation
```

Phases 1 and 2 can run in parallel since Phase 1 is purely Rust-side and Phase 2 is
purely TypeScript-side with no overlap.

---

## 6. What Each Phase Resolves

| Original Problem | Resolved by |
|-----------------|-------------|
| 4× grid-state clones per step | ✓ Done (Phase 0) |
| O(n²) rate history serialization | ✓ Done (Phase 0) |
| 30 `outputProfile*` derived | ✓ Done (Phase 0) |
| Hidden analytical Svelte components | ✓ Done (Phase 0) |
| TypeScript typecheck failures | ✓ Done (Phase 0.5) |
| O(n²) sweep efficiency derived | ✓ Done (Phase 1) |
| `analyticalMode` string routing in App.svelte | ✓ Done (Phase 2) |
| `liveAnalyticalOutput` param spreading | ✓ Done (Phase 2) |
| Styling constants duplicated | ✓ Done (Phase 3) |
| `referenceComparisonModel.ts` (2,560 lines) | ✓ Done (Phase 4) |
| `RateChart.svelte` curve-building inline (1,419 lines) | Phase 5 |
| `RateChart.svelte` hard-coded string matching | Phase 5 |
| Chart component not agnostic | Phase 5/6 |
| Store is a god object | Phase 6 |
| `activeScenarioAsFamily` in store | Phase 6 |
| App.svelte still has business logic | Phase 7 |
| Legacy benchmark layer duplication | Phase 8 |

---

## 7. Risk Assessment

| Phase | Risk | Mitigation |
|-------|------|-----------|
| 0.5 | Low — type narrowing fixes | Narrow carefully; run full typecheck after each file |
| 1 | Medium — Rust API change | Additive only; feature-detect in worker if needed; `cargo test` gate |
| 2 | Medium — all 10 scenarios touched | Port scenario-by-scenario; verify each analytical overlay |
| 3 | Low — mechanical constant extraction | Grep for all `borderDash` literals; remove one by one |
| 4 | High — 3,285-line file replaced | ✓ Done — four focused files; `referenceComparisonModel.ts` deleted |
| 5 | Medium — extract builder from RateChart | Keep `ChartSubPanel` working throughout; extract curves first, trim Svelte after |
| 6 | Medium — store split | Barrel re-export pattern; split one store at a time |
| 7 | Low — trim pass | By this point App.svelte logic is already moved elsewhere |
| 8 | Medium — legacy deletion | Ensure all scenarios pass reference runs; search for all import sites |

---

## 8. Files Created / Modified / Deleted Summary

### New files
| File | Phase | Purpose |
|------|-------|---------|
| `src/lib/analytical/index.ts` | 2 | Clean public API |
| `src/lib/analytical/axisAdapters.ts` | 2 | PVI→time, RF normalization |
| `src/lib/charts/panelDefs.ts` | 3 | Panel catalog (reusable across scenarios) |
| `src/lib/charts/curveStylePolicy.ts` | 3 | Single source for all styling constants |
| `src/lib/charts/referenceChartTypes.ts` | 4 ✓ | Types, colors, panel helpers |
| `src/lib/charts/referenceOverlayBuilders.ts` | 4 ✓ | BL / depletion / gas-oil overlays |
| `src/lib/charts/sweepPanelBuilder.ts` | 4 ✓ | Sweep efficiency panel builders |
| `src/lib/charts/buildChartData.ts` | 4 ✓ | Reference comparison chart data orchestrator |
| `src/lib/charts/buildRateChartData.ts` | 5 | Live rate chart data builder |
| `src/lib/charts/UniversalChart.svelte` | 5 | Scenario-agnostic chart renderer |
| `src/lib/stores/parameterStore.svelte.ts` | 6 | Simulation input parameters |
| `src/lib/stores/runtimeStore.svelte.ts` | 6 | Worker lifecycle + output |
| `src/lib/stores/navigationStore.svelte.ts` | 6 | Scenario/sensitivity/mode selection |

### Key modified files
| File | Phase | Change |
|------|-------|--------|
| `src/lib/catalog/scenarios.ts` | 2, 3 | Add `analyticalDef`, `chartPanels`; remove string-routing reliance |
| All 10 scenario files | 2, 3 | Populate `analyticalDef`, `chartPanels` |
| `src/lib/ressim/src/reporting.rs` | 1 | Add `sweep` field to `TimePointRates` |
| `src/lib/ressim/src/frontend.rs` | 1 | Add `setSweepGeometry()` |
| `src/App.svelte` | 0.5–7 | Progressively reduced; final target < 200 lines |
| `src/lib/charts/RateChart.svelte` | 4, 5 | Reduced to < 100 lines |
| `src/lib/charts/ReferenceComparisonChart.svelte` | 4, 5 | Reduced to < 200 lines |
| `src/lib/charts/ChartSubPanel.svelte` | 5 | Reduced to < 400 lines |

### Files to delete
| File | Phase | Reason |
|------|-------|--------|
| `src/lib/analytical/FractionalFlow.svelte` | ✓ WIP | Replaced by pure function |
| `src/lib/analytical/DepletionAnalytical.svelte` | ✓ WIP | Replaced by pure function |
| `src/lib/charts/referenceComparisonModel.ts` | 4 ✓ | Replaced by `buildChartData.ts` |
| `src/lib/stores/simulationStore.svelte.ts` | 6 | Split into three focused stores |
| `src/lib/catalog/benchmarkCases.ts` | 8 | Merged into scenario system |
| `src/lib/catalog/caseCatalog.ts` | 8 | Merged into scenario system |

---

## 9. Alignment with ROADMAP and TODO

| ROADMAP / TODO item | Addressed by |
|--------------------|-------------|
| ✓ Grid-state bundling + incremental rate history | Phase 0 (done) |
| ✓ Typed output-selection view model | Phase 0 (done) |
| ✓ Restore TypeScript typecheck health | Phase 0.5 (done) |
| Move sweep reporting into Rust | Phase 1 |
| Priority 2.1: Enforce one analytical method per scenario | Phase 2 |
| Priority 2.2: Finish sweep-method framework | Phase 1 + 2 |
| Priority 3.1: Collapse legacy benchmark layer | Phase 8 |
| Priority 3.2: Extract output-selection view model | ✓ Phase 0 (done) |
| Priority 1.1: SPE1 panel alignment (`cellDzPerLayer` normalization) | Phase 4 (`buildChartData.ts` fixes this) |
| Priority 1.3: Regression coverage gaps | Phase 8 |
