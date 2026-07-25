---
name: frontend-architecture
description: Navigate and modify the ResSim Svelte 5 frontend - charts, stores, workers, UI panels, 3D view. Use for any change under src/ outside the Rust core, especially chart/comparison work.
---

# Frontend Architecture & Change Guide

Svelte 5 + TypeScript strict + Tailwind 4 + Chart.js 4 + Three.js. The frontend is scenario-first end to end (2026-07-17): the legacy benchmark-family system was archived (see `.archive/README.md`), and the comparison chart stack's per-family physics is extracted into dedicated builder modules. Knowing which module owns which concern is still the main job — the stack just has fewer layers than it used to.

## Data flow

```
ScenarioPicker → parameterStore → buildCreatePayload → sim.worker.ts → WASM simulator
    → state snapshots (structured clone) → runtimeStore → charts / 3D view / diagnostics
```

- Stores (`src/lib/stores/`): `parameterStore.svelte.ts` (inputs), `runtimeStore.svelte.ts` (run results/playback), `navigationStore.svelte.ts` (mode/selection). All Svelte 5 runes classes. `simulationStore.svelte.ts` is a thin compatibility shim — don't add to it.
- Worker (`src/lib/workers/sim.worker.ts`): owns WASM lifecycle (explicit `initWasm()` gate before use). Messages must be **structured-cloneable**. Early-stop logic: `terminationPolicy.ts`.
- Scenario run model: `src/lib/scenario/runModel.ts` (run policy/specs per scenario).

## The chart stack (post-2026-07-17 consolidation)

| Layer | Files | Role |
|---|---|---|
| Scenario-first routing shell | `ScenarioChart.svelte`, `scenarioChartModel.ts` | Builds a `BenchmarkFamily`-shaped comparison spec from a `Scenario` (`buildScenarioComparisonFamily`), then routes to `ReferenceComparisonChart` (multi-run/sensitivity comparison) or `RateChart` (single live run). This is an intentional split by data shape, not an unfinished migration. |
| Comparison renderer | `ReferenceComparisonChart.svelte` (~590 ln), `buildChartData.ts` (~1600 ln, one cohesive `buildReferenceComparisonModel()` orchestrator) | Assembles the panel/curve model for the comparison view. Fully scenario-data-driven — the `BenchmarkFamily`/`BenchmarkVariant` *types_ it consumes live in `src/lib/scenario/referenceTypes.ts` (dependency-free), not in any legacy runtime data. |
| Analytical-method registry | `analyticalMethodRegistry.ts` | **The routing table.** One `AnalyticalMethodDescriptor` per `AnalyticalMethod`: curve slots (panel + curve key + per-context label), `fromResult` / `fromParams` overlay builders, overlay-mode rule, native x-axis, panel presentation, disclosure label. `buildChartData.ts`, `ReferenceComparisonChart.svelte`, `benchmarkDisclosure.ts` and `axisAdapters.ts` all read it instead of branching on the method name. |
| Per-family builder modules | `referenceOverlayBuilders.ts` (BL/depletion/gas-oil/well-test analytical overlays), `sweepPanelBuilder.ts` (sweep panels), `axisAdapters.ts` (x-axis mapping), `analyticalParamAdapters.ts` (per-method param derivation), `referenceChartTypes.ts` (shared panel/color types) | Each analytical family's physics/overlay logic lives here and is named by a registry entry — extend these when adding a new analytical method or overlay, don't inline more into the orchestrator. |
| Live single-run rendering | `RateChart.svelte` + `buildRateChartData.ts` | Separate path for the non-comparison (single active run) view. |

The legacy benchmark-family system (`bl_case_a_refined`, `dietz_sq_*`, `fetkovich_exp` — predates scenario-first) was confirmed unreachable from the live UI and archived to `.archive/` (not deleted — see `.archive/README.md` for how to resurrect). `src/lib/catalog/benchmarkCases.ts` is now a stub with the same exported names/types but empty data; nothing live depends on real data flowing through it. `caseCatalog.ts` is a **separate, still-live** system — Custom Mode's own facet/toggle catalog — with no remaining import coupling to `benchmarkCases.ts`.

Rules:

- **Adding an analytical method is a registry edit, not an orchestrator edit.** Write the overlay builder in `referenceOverlayBuilders.ts` / `analyticalParamAdapters.ts`, then add one `AnalyticalMethodDescriptor` to `analyticalMethodRegistry.ts`. Do not add an `if (analyticalMethod === …)` branch anywhere — if the behavior you need isn't expressible as a descriptor field, add the field. `analyticalMethodRegistry.test.ts` asserts a descriptor exists for every method and that native-axis declarations agree with `ANALYTICAL_OUTPUT_CONTRACTS`.
- A method with no reference solution (`'none'`, `'digitized-reference'`, `'sweep'`) has no curve slots and emits nothing. It must never fall through to another method's overlay path — that bug shipped for a while and is now pinned by a regression test in `referenceComparisonModel.test.ts`.
- **Sweep is an analytical method, not a flag.** There is no `showSweepPanel` capability to declare: `resolveCapabilities()` derives it from `analyticalMethod === 'sweep'`, and `sweepGeometry` is required on sweep and rejected elsewhere. There is no suppress/strip pass either — a sweep chart shows no primary analytical overlays because none are built.
- `validateScenarioChartLayout()` is a **positive** check: a layout may only name `-reference` curve keys its method actually emits. If it rejects your layout, either the method needs the slot or the key is dead — do not reintroduce a suppression mechanism.
- `buildChartData.ts`'s main orchestrator is cohesive control flow (mode resolution → per-result derived series → registry-driven analytical overlays → sweep panels → published overlays), not unextracted per-family logic. Extend it by adding a new sequential section for a new concern, following the existing pattern; don't inline analytical-method-specific physics here.
- `requiresRunMappedAnalyticalXAxis()` takes the solution's **native axis**, not the method name, so `axisAdapters.ts` stays a leaf module and cannot drift out of sync with the method list. Get the axis from the descriptor.
- Shared single-source-of-truth modules — extend these, don't fork them: `panelDefs.ts`, `curveStylePolicy.ts`, `chartLayouts.ts`.
- `CurveConfig[]` drives every panel. `toggleGroupKey` groups curves into legend toggle buttons; `legendSection`/`legendSectionLabel` group buttons under collapsible headers. Never bypass `ChartSubPanel.svelte`.
- Two independent visibility layers: `ReferenceComparisonChart` pre-filters by `visibleCaseKeys` (case visibility), then `ChartSubPanel` applies its own per-panel toggles. Don't merge or confuse them.
- `ReferenceSourceType` union (`'analytical' | 'published-reference' | 'opm-flow-precomputed' | 'simulation'`) is the right extension point for new reference sources.

## Other surfaces

- UI panels: `src/lib/ui/` (`modes/`, `sections/`, `controls/`, `cards/`, `feedback/`); composition contracts tested in `modePanel*.test.ts`.
- 3D: `src/lib/visualization/` + `spatialViewModel.ts`. Three.js is version-sensitive — **never upgrade it casually**; keep the exact pin in `package.json`.
- **Spatial vs run-results charts is the filing rule.** A run-results chart (`charts/`) shows one value per report step across a whole run. A spatial chart (`visualization/`) shows one value per cell at one instant and follows the 3D view's timestep and `showProperty` selectors — `SpatialProfileChart.svelte` + the pure `spatialProfileModel.ts`. Putting a snapshot view in `charts/` is what left the old `SwProfileChart` dormant: it had no timestep to follow.
- Analytical TS modules: `src/lib/analytical/` — these mirror engine physics; changing them may desync from Rust (see `engine-physics-change` skill).

## Coding rules

- **Svelte 5 runes only**: `$state`, `$derived`, `$derived.by`, `$effect`, `$props`, `$bindable`. No stores-as-`$`, no `export let`, no Svelte 4 patterns. Prefer `$derived.by` for multi-step derivations.
- Tailwind utilities, no custom CSS unless unavoidable.
- Single-purpose components; state as high as needed, no higher.
- No new dependencies without strong justification.
- Architecture-shaped tests exist and will fail on violations (e.g. `no-direct-chart-datasets-access.test.ts`, `ratechart-usage.test.ts`). Treat them as the spec.

## Validation

`pnpm run validate` (typecheck + lint + vitest + build). Visual check with `pnpm run dev` for anything user-visible. See `ressim-validation` skill.
