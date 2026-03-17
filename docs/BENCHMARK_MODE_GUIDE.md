# Benchmark Workflow Guide

Date: 2026-03-07; updated 2026-03-17
Status: **Reference guidance, sensitivity policy, and chart defaults remain current.** The "Source of truth" and "Execution workflow" sections below reference pre-simplification architecture (old `caseCatalog.ts` / `ModePanel.svelte` model) and will be rewritten after REFACTOR.md step 7 completes.

This page describes the benchmark system that the current frontend code and regression suite enforce. Use it together with `docs/P4_TWO_PHASE_BENCHMARKS.md`:

- this page explains how family-owned benchmark/reference workflows are organized and how the UI behaves
- `docs/P4_TWO_PHASE_BENCHMARKS.md` remains the reference for Buckley-Leverett breakthrough methodology and acceptance tolerances

## Source of truth

> **Note (2026-03-17):** The files listed below are being replaced by `src/lib/catalog/scenarios.ts` (scenario definitions + sensitivity variants) and `src/lib/ui/modes/ScenarioPicker.svelte` (selection UI). This section will be rewritten when REFACTOR.md step 7 completes. The physics, reference-guidance, sensitivity-policy, and chart-defaults sections above and below remain current.

The benchmark system is intentionally split by responsibility, with one logical owner for each concern:

- benchmark family definitions and generated variants live in `src/lib/catalog/benchmarkCases.ts`
- benchmark selector wiring and catalog exports live in `src/lib/catalog/caseCatalog.ts`
- normalized benchmark run specs and stored results live in `src/lib/benchmarkRunModel.ts`
- benchmark-specific chart defaults live in `src/lib/charts/referenceChartConfig.ts`
- benchmark comparison overlays live in `src/lib/charts/referenceComparisonModel.ts` and `src/lib/charts/ReferenceComparisonChart.svelte`
- family-owned benchmark/reference workflow UI lives in `src/lib/ui/modes/ModePanel.svelte` and `src/App.svelte`
- benchmark execution dispatch lives in `src/lib/stores/simulationStore.svelte.ts` through `runActiveReferenceSelection()`

There is no generated benchmark artifact pipeline. Benchmarks execute directly in browser-side WASM, and the authoritative validation evidence remains the Rust and frontend regression suites.

## Benchmark family model

A benchmark family owns:

- one base physical case
- a scenario class (`buckley-leverett` or `depletion`)
- zero or more supported sensitivity axes
- an explicit reference definition
- an optional benchmark comparison metric and tolerance
- default chart x-axis and panel choices
- benchmark display and run policy metadata

The current family inventory is:

| Scenario class | Family keys | Reference guidance | Sensitivity support |
|---|---|---|---|
| Buckley-Leverett | `bl_case_a_refined`, `bl_case_b_refined` | Buckley-Leverett reference solution for homogeneous runs; refined numerical reference for heterogeneity variants | Grid refinement, timestep refinement, heterogeneity |
| Depletion | `dietz_sq_center`, `dietz_sq_corner`, `fetkovich_exp` | Depletion reference solution | No generated sensitivity axes yet |

The Buckley-Leverett base families are aligned to the validated Rust benchmark semantics. That means the benchmark family is not just a UI preset label; it is intended to describe the same physical experiment as the benchmarked Rust case where parity is claimed.

## Sensitivity policy

The current Buckley-Leverett families generate variants from deltas rather than duplicated full payloads.

- `grid-refinement`: keeps the same physical model and Buckley-Leverett reference solution while changing spatial resolution
- `timestep-refinement`: keeps the same physical model and horizon while changing timestep size
- `heterogeneity`: introduces seeded permeability variation and switches the primary review baseline to a refined numerical reference

Important interpretation rule:

- homogeneous base, grid, and timestep runs can be judged against the Buckley-Leverett reference solution as the primary review baseline
- heterogeneous Buckley-Leverett variants should not be described as strict reference-solution-equality benchmarks; the Buckley-Leverett trace may still appear as secondary context, but the primary comparison is numerical-reference-based

## Execution workflow in the UI

Benchmark/reference workflows now live inside the owning product families rather than in a separate top-level benchmark destination.

The current workflow is:

1. Select the owning family in `Inputs`, then choose a benchmark/reference case from `Case Library`.
2. In the `Run` region's `Run Set`, choose either `Base case` or one sensitivity axis.
3. If an axis is selected, keep all variants checked or reduce the run to an explicit subset within that axis.
4. Run the selection through one button (`Run Base` or the axis-specific variant count label).
5. Review stored benchmark/reference results scoped to the active family in `Outputs`.
6. Optionally press `Customize` to seed the same family into editable custom state.

Current workflow constraints:

- the execution selector is axis-scoped: one axis is staged at a time
- base execution submits no variant keys; axis execution submits only the explicitly selected variant keys
- stored result cards are filtered to the active family so the workflow stays focused on one comparison set at a time

## Reference guidance and result semantics

Each stored benchmark run exposes explicit reference metadata instead of a generic summary string.

Benchmark results include:

- `referencePolicy`: what the primary review baseline is for the run
- `referenceComparison`: the current status of the benchmark metric against that reference
- `comparisonOutputs`: scenario-appropriate diagnostics such as breakthrough shift, recovery delta, final oil-rate error, cumulative oil error, or pressure delta

The current reference guidance behavior is:

- homogeneous Buckley-Leverett runs: the Buckley-Leverett reference solution is primary
- heterogeneous Buckley-Leverett runs: the refined numerical reference is primary, while the Buckley-Leverett trace remains secondary context
- depletion runs: the depletion reference solution is primary

This guidance is surfaced in benchmark summary cards and drives chart defaults and overlay composition.

## Chart defaults and comparison behavior

The benchmark workflow no longer reuses one generic single-run chart contract for every family.

### Buckley-Leverett defaults

- default x-axis: `PVI`
- alternate x-axis options: `time`, `cumInjection`
- default panels:
  - `Breakthrough`
  - `Recovery`
  - `Pressure`
- no log-scale toggle by default
- comparison chart overlays stored base-plus-variant runs and keeps the Buckley-Leverett reference-solution trace as shared context; for heterogeneous variants that trace is secondary context rather than the primary review metric

The reading order is intentionally breakthrough-first: water cut and average water saturation, then recovery/cumulative behavior, then pressure.

### Depletion defaults

- default x-axis: `time` for Dietz families, `logTime` for `fetkovich_exp`
- alternate x-axis options: `time`, `tD`, and `logTime` where supported
- default panels:
  - `Oil Rate`
  - `Cumulative Oil / Recovery`
  - `Pressure / Decline`
- depletion reference-solution traces remain the primary overlays

### Shared style policy

- color identifies case or variant
- line style identifies quantity or reference role
- pressure remains separated from the primary breakthrough panel to avoid mixed-unit clutter

## Validation and regression coverage

The benchmark system is protected at multiple layers:

- Rust benchmark methodology and tolerances: `docs/P4_TWO_PHASE_BENCHMARKS.md` and Rust tests under `src/lib/ressim`
- frontend Rust-parity regression: `src/lib/catalog/benchmarkPresetRuntime.test.ts`
- benchmark family and catalog integrity: `src/lib/catalog/caseCatalog.test.ts`
- benchmark run/result contract: `src/lib/benchmarkRunModel.test.ts`
- benchmark chart default policy: `src/lib/charts/referenceChartConfig.test.ts`
- benchmark multi-run overlay behavior: `src/lib/charts/referenceComparisonModel.test.ts`
- benchmark workflow wiring: `src/lib/ui/modePanelFlows.test.ts` and `src/lib/appStoreDomainWiring.test.ts`

If benchmark documentation and benchmark behavior diverge, update the documentation rather than relying on historical descriptions.