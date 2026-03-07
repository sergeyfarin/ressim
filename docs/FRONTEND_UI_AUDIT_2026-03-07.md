# Frontend UI Audit

Date: 2026-03-07
Status: current frontend/product audit for workflow, consistency, and usability

## Scope

This review covers the live frontend composition, frontend-facing state contracts, README/docs alignment, and the current backlog state.

Reviewed areas:

- app shell and page composition in `src/App.svelte`
- mode surfaces and section composition under `src/lib/ui/`
- chart composition under `src/lib/charts/`
- visualization workflow under `src/lib/visualization/`
- catalog/facet contracts under `src/lib/catalog/`
- warning policy and preset/customize contracts under `src/lib/`
- README, benchmark guide, docs index, and existing frontend review notes

## Executive Summary

The frontend has improved internal structure, but the product workflow is still harder to understand than it needs to be for a scientific application.

The main issue is not missing widgets. The main issue is that the app still behaves like several partially connected products:

- scenario selection and customization
- benchmark verification and sensitivity comparison
- run control and warning handling
- chart comparison and 3D inspection

Those pieces now have solid local implementations, but they do not yet form one coherent user journey.

## Strong Foundations Already Present

- The mode-panel architecture is materially cleaner than the older shell-style setup.
- Warning policy data is centralized in `src/lib/warningPolicy.ts` instead of being hand-built in multiple components.
- Benchmark definitions, reference policy, and chart defaults are much more explicit than before.
- The section/component split under `src/lib/ui/` is reasonable and gives a good base for a dedicated design pass.
- The faceted catalog is structured enough to support a more truthful product surface once constraints are clarified.

## Findings

### 1. Workflow and information architecture are still fragmented

Evidence:

- Benchmarks remain a separate top-level mode in `src/lib/ui/modes/ModePanel.svelte`.
- Benchmark mode focuses on family selection and execution-set selection in `src/lib/ui/modes/BenchmarkPanel.svelte`, but it does not expose the actual benchmark inputs in a readable case-summary form.
- The current benchmark UI still promotes `Clone to Custom`, which shifts the user out of the comparison workflow instead of strengthening the comparison workflow itself.
- Multi-run benchmark comparison exists in charts, but not in the 3D view or other output surfaces. `src/App.svelte` routes benchmark results into `BenchmarkChart`, while `ThreeDViewComponent` still receives only the single live runtime grid/history path.

Impact:

- Users must infer which mode is for building a case, which mode is for verification, and when to leave one mode for another.
- Benchmark users cannot inspect “what exactly is this case?” before running or comparing it.
- Sensitivity workflows stop at charts; they do not continue into spatial inspection or compact comparison summaries.

Recommendation:

- Make an explicit product decision about whether benchmarks stay separate or become embedded verification flows within depletion/waterflood.
- Replace mode-switching escape hatches with a clearer run-and-compare workflow.
- Add benchmark case disclosure cards that show key settings, reference policy, and variant deltas before execution.

### 2. Warning policy is unified in data but not unified in user experience

Evidence:

- `src/lib/warningPolicy.ts` centralizes warning construction, which is the right model.
- Warning surfaces are still split across the mode panel, run controls, and a separate analytical reference-caveat banner in `src/App.svelte`.
- This means the same warning system appears in multiple places with different scopes and no single “you must address this here” experience.

Impact:

- The app is technically consistent, but not experientially consistent.
- Users still have to scan multiple regions to understand whether an issue is blocking, non-physical, or only advisory.
- Section-level correction flow is weak because warning placement is not tied tightly enough to the relevant input group.

Recommendation:

- Keep one warning-policy model, but redesign the presentation into one canonical warning surface plus section-local error/status indicators.
- Link warnings back to input sections and fields instead of repeating warning groups in multiple page regions.

### 3. Chart architecture is only partially unified

Evidence:

- `src/lib/charts/RateChart.svelte` and `src/lib/charts/BenchmarkChart.svelte` both maintain local x-axis state, panel expansion state, and gutter alignment logic.
- Both charts implement similar cross-panel coordination, but through separate component-local state instead of a shared comparison/chart shell.

Impact:

- The current chart system works, but extension cost is higher than necessary.
- Any future work on synchronized panel state, output summaries, comparison controls, or responsive layout has to touch two top-level chart containers.
- This also makes “same interaction model across live run and benchmark compare” harder to deliver.

Recommendation:

- Consolidate shared comparison-shell behavior into a single chart container or shared chart-state layer.
- Keep scenario-specific panel content separate, but stop duplicating top-level interaction/state scaffolding.

### 4. Faceted selection is structured, but not product-complete or transparent

Evidence:

- `src/lib/catalog/catalog.json` exposes a broad set of dimensions for depletion, waterflood, and simulation.
- Several combinations are globally disabled by rule, including constraints that effectively make some surfaced options unavailable in practice.
- Waterflood explicitly disables areal/3D flooding with “Use Simulation for areal/3D flooding”, while the tabs and labels do not clearly explain this model split.

Impact:

- The UI looks more complete than it really is.
- Users can see a faceted surface but cannot easily tell which dimensions are core to a mode, which are intentionally restricted, and which belong in a different workflow.
- This weakens trust because “available in the catalog” and “supported in the product mental model” are not the same thing.

Recommendation:

- Audit each dimension per mode and decide whether it should be enabled, removed, or deliberately displayed as an advanced/incompatible option with a clear reason.
- Align labels and help text with the intended scenario families rather than just the internal mode split.

### 5. Input density is still too vertical for a scientific application

Evidence:

- Shared section bodies use `space-y-3 p-4 md:p-5` in `src/lib/ui/shared/panelStyles.ts`.
- Individual sections then add tables, inset cards, summaries, and checkbox rows on top of that default padding.
- Reservoir, wells, and relative-permeability sections rely heavily on vertically stacked table forms.

Impact:

- The page is readable, but too tall.
- Frequent tasks require more scrolling than necessary, especially when comparing or iterating on related parameters.
- Dense scientific inputs are not being grouped as compact workflows; they are being rendered as long form sections.

Recommendation:

- Move toward compact flowing cards/subcards, tighter spacing, and stronger desktop multi-column grouping.
- Keep full tables only where the user is genuinely working in a tabular mental model.

### 6. Visual design needs a dedicated product pass

Evidence:

- Theme tokens in `src/app.css` are functional, but dark mode is still anchored on near-black backgrounds.
- The page background uses layered geological decoration in `src/App.svelte` and `src/app.css` that adds atmosphere but also visual noise.

Impact:

- The app has character, but not enough restraint.
- Scientific content competes with decorative background treatment, especially on large screens.
- Light mode and dark mode both read more like theme toggles than carefully tuned working environments.

Recommendation:

- Rework both themes around calmer working surfaces and stronger panel contrast.
- Remove or significantly soften the reservoir-layer background treatment.
- Tighten margins and let data surfaces carry more of the page identity.

### 7. Label vocabulary and microcopy are inconsistent

Evidence:

- `Analytical Model` in `src/lib/ui/sections/AnalyticalSection.svelte` is ambiguous because it actually chooses a reference/solution mode.
- Typography and label sizing vary across section components.
- Terms like “Simulation”, “Benchmarks”, “Analytical Inputs”, “Clone to Custom”, and “Stored Benchmark Results” are individually understandable but do not form one vocabulary system.

Impact:

- The product sounds like it was assembled from several workstreams.
- Users have to interpret terminology instead of learning one consistent language.

Recommendation:

- Standardize label vocabulary across modes, charts, warnings, and summaries.
- Rename controls that reflect internal implementation choices rather than user intent.

## Recommended Execution Order

1. Decide the product workflow and information architecture first.
2. Unify warnings, labels, and benchmark disclosure around that workflow.
3. Unify chart/output architecture before adding more benchmark-specific presentation.
4. Add true multi-case comparison across 3D, charts, and summaries.
5. Run the compact-layout and theme pass after the interaction model is stable.
6. Refresh README/docs only after the UI language and workflow are settled.

## Mapping To Current User Concerns

1. Warnings are still not unified enough: confirmed.
2. Chart logic across different sections still has duplicated top-level behavior: confirmed.
3. `Copy/Clone to Custom` is probably over-promoted and should be re-justified: confirmed.
4. Benchmark settings are not disclosed well enough before or after run: confirmed.
5. Benchmarks being separate from depletion/waterflood is a product-design question that should be resolved explicitly: confirmed.
6. Depletion/waterflood faceted selection is not complete in product terms even where the catalog structure exists: confirmed.
7. Dark mode and light mode both need a more deliberate scientific-workspace design: confirmed.
8. Reservoir-layer background adds noise relative to its value: confirmed.
9. Labels need normalization across the product: confirmed.
10. Inputs are too tall and need denser grouping: confirmed.
11. Margins and paddings can be tightened materially: confirmed.
12. 3D view needs a multi-case comparison path for sensitivity runs: confirmed.
13. Sensitivity output summary needs a compact compare surface: confirmed.
14. Inputs, run/warnings, and outputs need clearer separation and stronger collapse behavior: confirmed, though the current layout already contains the raw building blocks.

## Deliverable For The Next Workstream

The next TODO should not be benchmark-only.

It should prioritize:

- workflow and information architecture
- warning and terminology consistency
- benchmark disclosure and comparison UX
- chart/output unification
- multi-case spatial comparison
- compact layout and theme polish
