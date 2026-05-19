# Frontend Architecture Rewrite Plan: Same Layout, Scenario-First Core

## Summary

The current frontend mixes three approaches: predefined scenarios, legacy benchmark families, and custom mode. This creates duplicated chart builders, confusing runtime paths, brittle tests, and unused or misleading case definitions. We will do a big internal rewrite while keeping the visible layout mostly the same: scenario picker, run controls, charts, and 3D view stay in familiar positions, but the data flow becomes scenario-first and IMPES-first.

Primary goals:

- Make predefined scenarios the canonical product model.
- Remove benchmark-family adapters from the app runtime path.
- Use one chart model for live IMPES, analytical, published, and OPM precomputed references.
- Split large stores/components into focused units.
- Make 3D visualization robust, typed, and easier to maintain.
- Keep FIM dev-only and out of frontend product decisions.

## Review Findings

- `App.svelte` currently renders `ReferenceComparisonChart` for nearly all predefined scenarios because `activeChartFamily` is synthesized from scenarios. As a result, `RateChart` and `liveChartPanels` are mostly bypassed for predefined scenarios and mainly affect custom mode, sometimes using the previous selected scenario.
- `navigationStore.svelte.ts` has too many responsibilities: scenario navigation, custom mode, benchmark-family adaptation, chart-family construction, analytical previews, output selection, 3D defaults, and sweep analytics.
- `runtimeStore.svelte.ts` mixes worker lifecycle, live run state, scenario sweep queueing, legacy reference-run queueing, convergence warning aggregation, and result hydration.
- `buildChartData.ts` is a 1600-line method-routed chart factory. It knows too much about BL, depletion, gas, sweep, published references, previews, layout, and styling.
- `ReferenceComparisonChart.svelte` duplicates axis and panel logic that already exists in `UniversalChart`/chart helpers.
- Scenario definitions are useful but too free-form: params are untyped `Record<string, unknown>`, large benchmark data lives inline, and validation does not prove chart curve keys, reference series, OPM artifact links, or well/grid bounds are coherent.
- `spe1LivePanels` is incomplete for the live chart path: BHP panel curves return `null`, injector/control-limit panels are absent from live panels, while the layout expects them.
- `3dview.svelte` is too large and still uses legacy Svelte `export let`/`$:` style. It also has concrete issues: pressure incremental history scan checks `Array.isArray(grid)` even though grid is an object, a duplicated `VisualReservoirMetrics` type appears inside a reactive block, `k ?? k` well indexing is duplicated, and camera FOV math appears wrong.
- Several tests assert source-code strings rather than behavior. One test matches `Customize` even though the visible button is commented out, so it can pass while the UI is broken.
- JSON-string signatures and `any` are used as glue in many places; acceptable temporarily, but not future-proof for a frontend that will add OPM artifacts and more case families.

## Key Changes

### 1. Canonical Scenario Model

- Introduce a single product-facing model:
  - `ScenarioDefinition`
  - `ScenarioCaseParams`
  - `ScenarioSensitivityDimension`
  - `ScenarioVariant`
  - `ScenarioReferenceSource`
  - `ScenarioChartDefinition`
  - `ScenarioRunPolicy`
- Treat legacy `BenchmarkFamily`, `BenchmarkRunSpec`, and case-library benchmark entries as migration-only compatibility types. They must not be used directly by `App.svelte`, chart components, or product runtime controls after the rewrite.
- Rename product runtime concepts away from benchmark wording:
  - `BenchmarkRunSpec` replacement: `RunSpec`
  - `BenchmarkRunResult` replacement: `RunResult`
  - `referenceSweepRunning` replacement: `runSetRunning`
  - `activeReferenceResults` replacement: `activeRunResults`
- Define reference source types explicitly:
  - `analytical`
  - `published-reference`
  - `opm-flow-precomputed`
  - `simulation`
- Move large SPE1 constants into dedicated data modules so the scenario file declares intent and references data, rather than embedding all PVT/SCAL/reference arrays inline.

### 2. Store Refactor

- Split `navigationStore.svelte.ts` into focused modules:
  - `scenarioSelectionStore`: selected scenario, dimension, variants, analytical option, custom mode.
  - `scenarioParamsStore` or existing `parameterStore`: parameter state and validation only.
  - `scenarioViewModel`: derived UI read models for picker, run controls, chart, and 3D.
  - `comparisonSelectionStore`: selected run result for chart/3D comparison.
- Split `runtimeStore.svelte.ts`:
  - `workerRuntimeStore`: WASM worker setup, create/run/stop, live history.
  - `runSetExecutor`: queue execution for scenario variants using `RunSpec`.
  - `runResultBuilder`: converts worker output to `RunResult`.
  - `runtimeWarningsStore`: convergence/runtime warning aggregation.
- Remove circular `runtimeStore` to `navigationStore` dependency. Runtime receives explicit `RunSpec`/payload inputs and emits results; scenario state decides what to run.

### 3. Unified Chart Architecture

- Replace the split between `RateChart` and `ReferenceComparisonChart` with one chart entrypoint:
  - `ScenarioChart.svelte`
  - input: `ChartModel`
  - output: pure panel rendering through `ChartSubPanel`.
- Define a generic chart model:
  - `ChartModel`: x-axis options, selected axis, panels, warnings, case toggles.
  - `ChartPanelModel`: key, title, curves, scale preset, default expanded, visible.
  - `ChartCurveModel`: source type, source id, label, style role, y-axis, data.
- Move domain-specific curve construction into small builders:
  - `waterfloodChartBuilder`
  - `depletionChartBuilder`
  - `gasChartBuilder`
  - `sweepChartBuilder`
  - `publishedReferenceBuilder`
  - `opmArtifactChartBuilder`
- Keep `ChartSubPanel` as the Chart.js host, but make it receive a stable panel model and avoid parent/child feedback loops. Any measured gutter state must update only when values materially change.
- Remove direct chart behavior from scenario files. Scenario definitions should declare chart intent and supported outputs; chart builders convert results/references into curves.

### 4. Same Layout UI Cleanup

- Keep the current page shape:
  - header
  - scenario picker
  - run controls
  - charts on the left
  - 3D visualization on the right
- Internally split `App.svelte` into small shell components:
  - `ScenarioPane`
  - `RunPane`
  - `ResultsPane`
  - `SpatialPane`
- Rework `ScenarioPicker.svelte` into smaller pieces:
  - scenario family selector
  - scenario summary
  - analytical option selector
  - sensitivity selector
  - variant selector
  - custom parameter editor launcher/section
- Restore or remove the customize action explicitly. Do not leave commented UI that tests still treat as present.
- Keep public copy IMPES-first. Do not show FIM labels, solver selectors, or FIM warnings in product UI.

### 5. 3D Visualization Refactor

- Split `3dview.svelte` into:
  - `SpatialView.svelte`: Svelte UI shell and bindings.
  - `threeReservoirRenderer.ts`: Three.js scene, mesh, camera, resize, disposal.
  - `spatialViewModel.ts`: active grid/well/history selection and validation.
  - `spatialColorScales.ts`: pressure/saturation/ternary color and legend logic.
- Convert to Svelte 5 props/runes consistently.
- Fix known issues:
  - use object-shaped `GridState` in pressure history scan.
  - remove duplicated nested type declaration.
  - fix camera FOV conversion to degrees × `Math.PI / 180`.
  - fix duplicated `k ?? k` well indexing.
  - clamp invalid current index and disable slider when no history exists.
  - replace HTML tooltip string assembly with structured fields rendered by Svelte.
- Add a clear display mode for boundary-shell visualization, since only boundary cells are rendered.

### 6. Case Definition Cleanup

- Add scenario validation that fails tests when:
  - scenario params are missing required simulator fields.
  - well indices are outside grid bounds for base or variants.
  - variant patches change grid size without updating producer/injector indices.
  - chart curve keys reference missing builder outputs.
  - `affectsAnalytical` disagrees with analytical input fingerprints.
  - OPM artifact keys do not exist or point to the wrong scenario.
  - published reference series use unknown panel keys.
- Create shared scenario param factories:
  - `baseWaterfloodParams`
  - `baseSweepParams`
  - `baseDepletionParams`
  - `baseGasParams`
  - `spe1Params`
- Keep scenario files readable: label, intent, params factory call, sensitivities, references, run policy.
- Keep OPM Flow artifacts precomputed and explicit. The browser never runs OPM Flow.

## Test Plan

- Product gates:
  - `npm run typecheck`
  - `npm run lint`
  - `npm test`
  - `npm run build`
  - IMPES Rust coverage gate
- Add scenario contract tests:
  - every predefined scenario defaults to `fimEnabled: false`.
  - every scenario and variant builds a valid `RunSpec`.
  - every `RunSpec` builds a valid simulator payload.
  - variant well/grid bounds are valid.
  - chart builders produce panels with non-empty curves for each scenario.
  - reference source metadata is present and rendered correctly.
- Replace brittle source-string UI tests with behavioral tests around pure view models and component props.
- Add chart model tests:
  - IMPES simulation vs analytical curves can coexist.
  - IMPES simulation vs published references can coexist.
  - IMPES simulation vs OPM artifact curves can coexist.
  - preview, running, completed, and partial run-set states keep stable case colors and toggles.
- Add 3D tests for extracted pure modules:
  - active grid selection.
  - pressure range from history.
  - saturation range.
  - layer thickness metrics.
  - well column placement.
  - invalid history/grid fallback.
- Keep FIM tests opt-in only and outside product readiness validation.

## Migration Order

1. Add new neutral types and pure builders while leaving old UI running.
2. Build `RunSpec`/`RunResult` from scenarios without benchmark-family adapters.
3. Replace scenario execution path in runtime with `runSetExecutor`.
4. Replace `ReferenceComparisonChart`/`RateChart` branching with `ScenarioChart`.
5. Refactor `App.svelte` into the same-layout shell components.
6. Refactor 3D visualization into renderer/view-model/color modules.
7. Move SPE1 and other bulky case data into dedicated data modules.
8. Remove old benchmark-family app adapters once scenario tests and product gates pass.
9. Keep legacy benchmark files only if still needed for historical tests/docs; otherwise archive or delete.

## Assumptions

- “Big rewrite” means broad internal replacement is acceptable, but the visible layout should remain mostly familiar.
- IMPES remains the only product solver path.
- FIM remains dev-only and must not shape frontend UX.
- OPM Flow references are precomputed artifacts, never browser-executed.
- No new runtime dependency is required for schema validation unless already present; use TypeScript types plus explicit validation helpers first.
- Existing `uv` rule remains permanent for Python tooling.

## Progress


I started implementation and added the first scenario-first module:

New: runModel.ts
It defines RunSpec, RunResult, ScenarioReferenceSource, ScenarioRunPolicy, scenario-native run-spec creation, create-payload bridging, and result-building wrappers.
I then hit a tooling blocker: file writes started failing with:

bwrap: loopback: Failed RTM_NEWADDR: Operation not permitted

Escalation was also rejected by the environment, so I stopped before partially rewiring runtimeStore, navigationStore, App.svelte, and charts. Current worktree has only:

?? src/lib/scenario/

No tests were run after this partial change, because the app is not meaningfully rewired yet. Once the sandbox write path is available again, the next implementation step is to wire runtimeStore to RunSpec/RunResult, then replace the App.svelte chart branch with a single ScenarioChart.svelte entrypoint.
