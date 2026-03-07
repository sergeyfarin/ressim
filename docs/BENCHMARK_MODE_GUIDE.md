# Benchmark Mode Guide

Date: 2026-03-07
Status: authoritative guide for the current benchmark registry, execution workflow, reference policy, and chart behavior

This page describes the benchmark system that the current frontend code and regression suite enforce. Use it together with `docs/P4_TWO_PHASE_BENCHMARKS.md`:

- this page explains how benchmark mode is organized and how the UI behaves
- `docs/P4_TWO_PHASE_BENCHMARKS.md` remains the reference for Buckley-Leverett breakthrough methodology and acceptance tolerances

## Source of truth

The benchmark system is intentionally split by responsibility, with one logical owner for each concern:

- benchmark family definitions and generated variants live in `src/lib/catalog/benchmarkCases.ts`
- benchmark selector wiring and catalog exports live in `src/lib/catalog/caseCatalog.ts`
- normalized benchmark run specs and stored results live in `src/lib/benchmarkRunModel.ts`
- benchmark-specific chart defaults live in `src/lib/charts/benchmarkChartConfig.ts`
- benchmark comparison overlays live in `src/lib/charts/benchmarkComparisonModel.ts` and `src/lib/charts/BenchmarkChart.svelte`
- benchmark mode workflow UI lives in `src/lib/ui/modes/BenchmarkPanel.svelte`
- benchmark execution dispatch lives in `src/lib/stores/simulationStore.svelte.ts` through `runActiveBenchmarkSelection()`

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

| Scenario class | Family keys | Reference policy | Sensitivity support |
|---|---|---|---|
| Buckley-Leverett | `bl_case_a_refined`, `bl_case_b_refined` | Analytical Buckley-Leverett shock reference for homogeneous runs; refined numerical reference for heterogeneity variants | Grid refinement, timestep refinement, heterogeneity |
| Depletion | `dietz_sq_center`, `dietz_sq_corner`, `fetkovich_exp` | Analytical depletion reference | No generated sensitivity axes yet |

The Buckley-Leverett base families are aligned to the validated Rust benchmark semantics. That means the benchmark family is not just a UI preset label; it is intended to describe the same physical experiment as the benchmarked Rust case where parity is claimed.

## Sensitivity policy

The current Buckley-Leverett families generate variants from deltas rather than duplicated full payloads.

- `grid-refinement`: keeps the same physical model and analytical reference while changing spatial resolution
- `timestep-refinement`: keeps the same physical model and horizon while changing timestep size
- `heterogeneity`: introduces seeded permeability variation and switches the primary truth source to a refined numerical reference

Important interpretation rule:

- homogeneous base, grid, and timestep runs can be judged against the analytical Buckley-Leverett reference as the primary truth source
- heterogeneous Buckley-Leverett variants should not be described as strict analytical-equality benchmarks; analytical traces may still appear as secondary context, but the primary comparison is numerical-reference-based

## Execution workflow in the UI

Benchmark mode is a benchmark-family runner and comparison surface, not a generic preset launcher.

The current workflow is:

1. Select a benchmark family.
2. In `Execution Set`, choose either `Base case` or one sensitivity axis.
3. If an axis is selected, keep all variants checked or reduce the run to an explicit subset within that axis.
4. Run the selection through one button (`Run Base` or the axis-specific variant count label).
5. Review stored benchmark results scoped to the active family.
6. Optionally clone the selected benchmark into custom mode for ad hoc editing.

Current workflow constraints:

- the execution selector is axis-scoped: one axis is staged at a time
- base execution submits no variant keys; axis execution submits only the explicitly selected variant keys
- stored result cards are filtered to the active family so the workflow stays focused on one comparison set at a time

## Reference policy and result semantics

Each stored benchmark run exposes explicit reference metadata instead of a generic summary string.

Benchmark results include:

- `referencePolicy`: what the primary truth source is for the run
- `referenceComparison`: the current status of the benchmark metric against that reference
- `comparisonOutputs`: scenario-appropriate diagnostics such as breakthrough shift, recovery delta, final oil-rate error, cumulative oil error, or pressure delta

The current reference policy behavior is:

- homogeneous Buckley-Leverett runs: analytical Buckley-Leverett is primary, analytical overlay is primary
- heterogeneous Buckley-Leverett runs: refined numerical reference is primary, analytical overlay is secondary
- depletion runs: analytical depletion reference is primary

This policy is surfaced in benchmark summary cards and drives chart defaults and overlay composition.

## Chart defaults and comparison behavior

Benchmark mode no longer reuses one generic single-run chart contract for every family.

### Buckley-Leverett defaults

- default x-axis: `PVI`
- alternate x-axis options: `time`, `cumInjection`
- default panels:
  - `Breakthrough`
  - `Recovery`
  - `Pressure`
- no log-scale toggle by default
- comparison chart overlays stored base-plus-variant runs and keeps the analytical Buckley-Leverett trace as shared context; for heterogeneous variants that trace is secondary context rather than the primary truth metric

The reading order is intentionally breakthrough-first: water cut and average water saturation, then recovery/cumulative behavior, then pressure.

### Depletion defaults

- default x-axis: `time` for Dietz families, `logTime` for `fetkovich_exp`
- alternate x-axis options: `time`, `tD`, and `logTime` where supported
- default panels:
  - `Oil Rate`
  - `Cumulative Oil / Recovery`
  - `Pressure / Decline`
- analytical depletion traces remain primary reference overlays

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
- benchmark chart default policy: `src/lib/charts/benchmarkChartConfig.test.ts`
- benchmark multi-run overlay behavior: `src/lib/charts/benchmarkComparisonModel.test.ts`
- benchmark workflow wiring: `src/lib/ui/modePanelFlows.test.ts` and `src/lib/appStoreDomainWiring.test.ts`

If benchmark documentation and benchmark behavior diverge, update the documentation rather than relying on historical descriptions.