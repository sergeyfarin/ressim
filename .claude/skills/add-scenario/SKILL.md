---
name: add-scenario
description: Add a new predefined scenario/case to the ResSim catalog, or add sensitivity dimensions and analytical overlays to an existing one. Use when adding teaching cases, benchmark cases, sensitivity studies, or wiring a new analytical reference method into the UI.
---

# Adding a Scenario to the Catalog

Scenarios are the product's core content unit: a self-describing definition that drives the picker, simulation parameters, sensitivity sweeps, analytical overlays, charts, and 3D view. Candidate cases and sourcing: `docs/CASE_LIBRARY_ROADMAP.md`.

## Before you start

- Pick the analytical reference **first** and be honest about its assumptions (project principle: "analytical methods only where assumptions remain explicit and defensible"). If no quantitative reference exists, use `analyticalMethod: 'none'` and say so in the description — `gas_drive` is the precedent.
- Check physics support: 3D Cartesian grid only (no radial/LGR), two-phase O/W validated, three-phase O/W/G experimental, black-oil PVT available, no aquifer models, no well schedules. Don't define a scenario the engine can't honestly run.
- Read one exemplar end-to-end: `src/lib/catalog/scenarios/wf_bl1d.ts` (simple) or `spe1_gas_injection.ts` (black-oil, per-layer dz, PVT table, published references).

## Checklist

1. **Create** `src/lib/catalog/scenarios/<key>.ts` exporting a `Scenario`.
   - Key convention: `{domain}_{physics_descriptor}` (`wf_bl1d`, `dep_pss`, `gas_injection`).
   - Required: `key`, `label`, `catalog` (explicit group, role, case mode, picker summary), `description`, `analyticalMethodSummary`, `analyticalMethodReference` (cite the actual literature), `chartLayoutKey`, `capabilities`, scenario-owned `solverPolicy`, `params`, `analyticalDef`, `liveChartPanels`, `sensitivities`, `defaultSensitivityDimensionKey`.
   - `capabilities` routes everything: `analyticalMethod`, `hasInjector`, `default3DScalar`, `requiresThreePhaseMode`. It is a **discriminated union on `analyticalMethod`**, so the compiler enforces the method's own rules: `primaryRateCurve` is narrowed to that method's `supportedRateCurves`, and `sweepGeometry` (plus optional `sweepMethods`) is required on `'sweep'` and rejected on every other method (there is no `showSweepPanel` flag). If an editor rejects your capabilities object, the scenario is wrong — don't cast around it.
2. **Register** it in `src/lib/catalog/scenarios.ts` (import + registry entry). That file is the single source of truth.
3. **Analytical wiring**: reuse an existing `analyticalDef` from `src/lib/catalog/analyticalAdapters.ts` if the method exists. A genuinely new method needs, in order — this is a bigger change; keep it a separate commit:
   - the math in `src/lib/analytical/` with its own unit tests against known values;
   - an adapter in `analyticalAdapters.ts` (live path) and an overlay builder in `charts/referenceOverlayBuilders.ts` + a `compute…FromParams` in `charts/analyticalParamAdapters.ts` (comparison path);
   - an entry in the `AnalyticalMethod` union and in `ANALYTICAL_OUTPUT_CONTRACTS` (`catalog/scenarios.ts`);
   - **one `AnalyticalMethodDescriptor` in `charts/analyticalMethodRegistry.ts`** — curve slots, overlay builders, overlay-mode rule, native x-axis, panel presentation, disclosure label.

   That last entry is the whole comparison-chart wiring. Do **not** add an `if (analyticalMethod === …)` branch to `buildChartData.ts`, `ReferenceComparisonChart.svelte` or `benchmarkDisclosure.ts` — they are registry-driven. See the `frontend-architecture` skill.
4. **Charts**: pick or compose `liveChartPanels` from `src/lib/catalog/chartPanels/`; pick `chartLayoutKey` from `chartLayouts.ts`. Follow the `CurveConfig[]` / `toggleGroupKey` / `legendSection` pattern — never bypass `ChartSubPanel`.
   - All case-specific panel choices, curve selections, expansion state, labels and formatting overrides belong in the scenario's `chartLayoutPatch`; renderers must not branch on the scenario key.
5. **Sweep scenarios**: list selectable correlations in `capabilities.sweepMethods`, most-preferred first (first = default). Labels, claims and citations come from `src/lib/analytical/sweepMethods.ts` — never restate them in the scenario file, and never add a `Scenario.analyticalOptions` array (the field is gone). Only declare two or more methods where the choice actually changes the curves: at `sweepGeometry: 'areal'` the correlations are numerically identical, so a toggle there would do nothing. Measure before offering one.
6. **Sensitivities**: dimension keys are `lower_snake`; variant keys are `{dim_abbrev}_{value_tag}` (`mob_favorable`, `sor_low`). Every variant needs a `paramPatch` and an honest `affectsAnalytical` flag.
   - `affectsAnalytical: true` is **test-enforced**: a contract test verifies the patch actually perturbs the analytical result. A variant that only changes `steps` or grid must be `false`.
   - `analyticalOverlayMode`: `'per-result'` when the analytical curve changes per variant, `'shared'` for grid-convergence-style studies where the analytical reference is fixed.
7. **Termination**: if the scenario should stop early (breakthrough, pressure floor), see `docs/SCENARIO_TERMINATION_POLICY.md`.
8. **Optional references**: one declared `referenceSources` list — `{ kind: 'published', series }` for digitized paper data, `{ kind: 'opm-flow', artifactKeys }` for bundled OPM Flow ground truth (add `role: 'primary'` when the artifact *is* the exhibit, i.e. `runMode: 'prerun-artifacts'`). Sources render in declaration order. Nothing is matched implicitly — an artifact appears only if the scenario names it. See `spe1_gas_injection.ts` (both kinds) and the `opm-reference-pipeline` skill.
9. **Docs**: add a row to the scenario inventory table in `README.md`.
10. **Architecture**: a new top-level picker entry needs a distinct engineering question and a plot that answers it. A new reference source belongs on an existing scenario, and a parameter variation belongs in a sensitivity dimension. Never add a canonical scenario-key conditional outside `catalog/scenarios/`; `scenarioAgnosticArchitecture.test.ts` enforces this.

## Validation

```bash
pnpm run typecheck
pnpm test        # scenarios.test.ts, caseLibrary.test.ts, analytical contract tests, chart model tests
pnpm run lint
```

Then run it visually: `pnpm run dev`, select the scenario, run base + each sensitivity variant, and check: simulation completes, analytical overlay is present and plausibly close, sensitivity variants move curves in the physically expected direction, 3D view shows the intended property.

## Quality bar for a case to be "done"

- Analytical overlay quantitatively sensible (breakthrough timing / decline rate / recovery within the tolerance philosophy of `docs/P4_TWO_PHASE_BENCHMARKS.md`), or explicitly documented as qualitative.
- Each sensitivity dimension teaches one identifiable reservoir-engineering concept, stated in its `description`.
- Descriptions name the method's validity limits (e.g. "Dykstra-Parsons assumes non-communicating layers").
- If the case comes from a book/paper/dataset, the citation is in `analyticalMethodReference` and the parameter provenance is reproducible.
