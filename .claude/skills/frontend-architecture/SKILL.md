---
name: frontend-architecture
description: Navigate and modify the ResSim Svelte 5 frontend - charts, stores, workers, UI panels, 3D view. Use for any change under src/ outside the Rust core, especially chart/comparison work, and to avoid growing the legacy chart layers that are slated for removal.
---

# Frontend Architecture & Change Guide

Svelte 5 + TypeScript strict + Tailwind 4 + Chart.js 4 + Three.js. The frontend is mid-consolidation: a scenario-first architecture has landed, but legacy layers are still live. Knowing which layer to touch is most of the job.

## Data flow

```
ScenarioPicker → parameterStore → buildCreatePayload → sim.worker.ts → WASM simulator
    → state snapshots (structured clone) → runtimeStore → charts / 3D view / diagnostics
```

- Stores (`src/lib/stores/`): `parameterStore.svelte.ts` (inputs), `runtimeStore.svelte.ts` (run results/playback), `navigationStore.svelte.ts` (mode/selection). All Svelte 5 runes classes. `simulationStore.svelte.ts` is a thin compatibility shim — don't add to it.
- Worker (`src/lib/workers/sim.worker.ts`): owns WASM lifecycle (explicit `initWasm()` gate before use). Messages must be **structured-cloneable**. Early-stop logic: `terminationPolicy.ts`.
- Scenario run model: `src/lib/scenario/runModel.ts` (run policy/specs per scenario).

## The chart stack — three coexisting generations (critical)

| Generation | Files | Status |
|---|---|---|
| New scenario-first | `ScenarioChart.svelte`, `scenarioChartModel.ts` | The intended future — currently a thin shell |
| Phase-4/5 era | `ReferenceComparisonChart.svelte` (~590 ln), `buildChartData.ts` (~1600 ln), `RateChart.svelte` + `buildRateChartData.ts` | **Still does the real work** |
| Legacy benchmark | `benchmarkCases.ts`, `caseCatalog.ts`, `ReferenceExecutionCard` | Slated for removal, still imported |

Rules:

- **Do not add new features to `buildChartData.ts` or the legacy benchmark files** unless there is no alternative; if you must, note it in `TODO.md`. The consolidation direction (ROADMAP Priority 3, `docs/COMPARISON_TOOLBOX_REVIEW_2026-07-01.md` Phase B) is: fold everything into the scenario-first path.
- Shared single-source-of-truth modules that BOTH old and new paths use — extend these, don't fork them: `panelDefs.ts`, `curveStylePolicy.ts`, `chartLayouts.ts`.
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
