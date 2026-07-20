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
| Per-family builder modules | `referenceOverlayBuilders.ts` (BL/depletion/gas-oil analytical overlays), `sweepPanelBuilder.ts` (sweep panels), `axisAdapters.ts` (x-axis mapping), `analyticalParamAdapters.ts` (per-method param derivation), `referenceChartTypes.ts` (shared panel/color types) | Each analytical family's physics/overlay logic already lives here, called by `buildChartData.ts` — extend these when adding a new analytical method or overlay, don't inline more into the orchestrator. |
| Live single-run rendering | `RateChart.svelte` + `buildRateChartData.ts` | Separate path for the non-comparison (single active run) view. |

The legacy benchmark-family system (`bl_case_a_refined`, `dietz_sq_*`, `fetkovich_exp` — predates scenario-first) was confirmed unreachable from the live UI and archived to `.archive/` (not deleted — see `.archive/README.md` for how to resurrect). `src/lib/catalog/benchmarkCases.ts` is now a stub with the same exported names/types but empty data; nothing live depends on real data flowing through it. `caseCatalog.ts` is a **separate, still-live** system — Custom Mode's own facet/toggle catalog — with no remaining import coupling to `benchmarkCases.ts`.

Rules:

- `buildChartData.ts`'s main orchestrator is long (~1,190 lines) but cohesive control flow (mode resolution → per-result derived series → per-analytical-method curve assembly → sweep panels → published overlays), not unextracted per-family logic — that's already split into the builder modules above. Extend it by adding a new sequential section for a new concern, following the existing pattern; don't inline analytical-method-specific physics here.
- Shared single-source-of-truth modules — extend these, don't fork them: `panelDefs.ts`, `curveStylePolicy.ts`, `chartLayouts.ts`.
- `CurveConfig[]` drives every panel. `toggleGroupKey` groups curves into legend toggle buttons; `legendSection`/`legendSectionLabel` group buttons under collapsible headers. Never bypass `ChartSubPanel.svelte`.
- Two independent visibility layers: `ReferenceComparisonChart` pre-filters by `visibleCaseKeys` (case visibility), then `ChartSubPanel` applies its own per-panel toggles. Don't merge or confuse them.
- `ReferenceSourceType` union (`'analytical' | 'published-reference' | 'opm-flow-precomputed' | 'simulation'`) is the right extension point for new reference sources.

## Other surfaces

- UI panels: `src/lib/ui/` (`modes/`, `sections/`, `controls/`, `cards/`, `feedback/`); composition contracts tested in `modePanel*.test.ts`.
- 3D: `src/lib/visualization/` + `spatialViewModel.ts`. Three.js is version-sensitive — **never upgrade it casually**; keep the exact pin in `package.json`.
- Analytical TS modules: `src/lib/analytical/` — these mirror engine physics; changing them may desync from Rust (see `engine-physics-change` skill).

## Coding rules

- **Svelte 5 runes only**: `$state`, `$derived`, `$derived.by`, `$effect`, `$props`, `$bindable`. No stores-as-`$`, no `export let`, no Svelte 4 patterns. Prefer `$derived.by` for multi-step derivations.
- Tailwind utilities, no custom CSS unless unavoidable.
- Single-purpose components; state as high as needed, no higher.
- No new dependencies without strong justification.
- Architecture-shaped tests exist and will fail on violations (e.g. `no-direct-chart-datasets-access.test.ts`, `ratechart-usage.test.ts`). Treat them as the spec.

## Validation

`pnpm run validate` (typecheck + lint + vitest + build). Visual check with `pnpm run dev` for anything user-visible. See `ressim-validation` skill.
