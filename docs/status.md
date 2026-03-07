## Current Snapshot (2026-03-07)

- Benchmark presets now resolve from a single frontend benchmark registry instead of duplicated catalog payloads.
- Legacy named scenario cases now resolve from a typed frontend preset registry in `src/lib/catalog/presetCases.ts`; the old `public/cases` manifest/artifact bundle has been removed from the app source path.
- Benchmark-family schema and ownership are now implemented in code:
  - family metadata, selector options, and runtime benchmark entries resolve from one registry contract
  - benchmark parameter payloads now come from source JSON files under `src/lib/catalog/benchmark-case-data/`, not from `public/` imports or duplicated blobs
- B2 is now implemented for the Buckley-Leverett base families:
  - refined BL source cases now use Rust-parity pressure-controlled semantics instead of the old rate-controlled frontend variant
  - benchmark-family metadata now records the breakthrough criterion and accepted breakthrough-PV comparison tolerance
  - the worker now applies authored uniform permeability values instead of silently leaving constructor defaults in place
- B3 is now implemented for the Buckley-Leverett registry layer:
  - BL sensitivity variants are generated from family deltas in `src/lib/catalog/benchmarkCases.ts` instead of duplicated case payloads
  - the initial generated suite covers grid refinement, timestep refinement, and curated seeded heterogeneity variants
  - heterogeneity variants now carry an explicit numerical-reference requirement in their generated metadata
- B4 is now implemented for the benchmark runner path:
  - benchmark families can execute as base-only or base-plus-variants serial sweeps through the existing worker/store path
  - normalized benchmark results now record breakthrough, water-cut, pressure, recovery, and reference-comparison outputs per run
  - benchmark mode now surfaces sweep progress, cancellation, and stored-result summaries without reintroducing duplicated benchmark payloads
- B5 is now implemented for explicit reference handling:
  - benchmark run results now carry an explicit reference-policy contract instead of relying on an ambiguous generic summary string
  - homogeneous BL runs identify analytical Buckley-Leverett as the primary truth source, while heterogeneous BL runs identify refined numerical reference as primary and only secondary analytical context
  - depletion runs identify analytical depletion reference explicitly and report trend-based diagnostics rather than pretending a breakthrough-style metric applies
- The remaining benchmark mismatch is presentation-related, not reference-contract-related:
  - BL and depletion benchmark runs now expose what “reference” means directly in the result contract and benchmark summary UI
  - the next step is using that explicit contract to drive benchmark-specific chart composition and defaults
- The next workstream is benchmark modernization: benchmark-specific charts, multi-case overlay plumbing, and the later benchmark UI workflow pass.

## Legacy Cleanup

- Removed legacy Phase 1 / Phase 2 execution history from this live status file.
- Historical implementation detail remains available in git history and older benchmark/docs pages; this file now tracks only the current benchmark modernization state.
- Curated long-term options from the old backlog were reintroduced in `TODO.md` as a separate retained-options section, then classified into `Important Later` and `Nice To Have Only` with duplicates removed.

## Proposed End Objective (pending review)

Build a benchmark system with these properties:

- one logical source of truth for benchmark definitions
- exact Rust parity for benchmark-family base physics where applicable
- generated sensitivity variants instead of duplicated full-case payloads
- benchmark-specific chart layouts and defaults
- explicit analytical-vs-numerical reference policy
- readable multi-case comparison workflow in the frontend

## Proposed Benchmark Modernization Plan

### B0. Plan review and sign-off

Current state:
- pending user review before implementation starts

Goal:
- confirm the target objective, sensitivity scope, and chart policy before modifying benchmark semantics

### B1. Benchmark-family schema and ownership

Goal:
- define one benchmark registry contract that owns base physics, variants, reference policy, display defaults, and run policy

Key outcome:
- benchmark selector data and benchmark execution data come from the same source

Status:
- completed on 2026-03-07
- implementation lives in `src/lib/catalog/benchmarkCases.ts` and is re-exported through `src/lib/catalog/caseCatalog.ts`
- current BL families already declare intended sensitivity axes, reference kind, display defaults, style policy, and run policy metadata

### B2. Align BL families to exact Rust semantics

Goal:
- make BL benchmark families physically equivalent to the Rust benchmark builders

Key outcome:
- frontend BL benchmark means the same experiment as the validated Rust benchmark

Status:
- completed on 2026-03-07
- refined BL benchmark source files now use pressure-controlled Rust-parity settings
- benchmark-family metadata now records `watercut >= 1%` breakthrough detection and accepted breakthrough-PV relative-error tolerance
- wasm-backed runtime regression now checks breakthrough-PV alignment against the declared analytical reference and tolerance

### B3. Generate sensitivity variants from base families

Goal:
- support meaningful BL sensitivity studies without duplicating complete case payloads

Initial axes:
- grid refinement
- timestep refinement
- curated heterogeneity variants

Key outcome:
- one family definition can produce base and sensitivity runs deterministically

Status:
- completed on 2026-03-07
- BL sensitivity variants now resolve from delta-based generation in `src/lib/catalog/benchmarkCases.ts`
- the initial generated suite covers grid refinement, timestep refinement, and curated seeded heterogeneity variants
- heterogeneous generated variants now declare numerical-reference-required comparison metadata instead of silently inheriting the homogeneous analytical contract

### B4. Multi-run benchmark execution model

Goal:
- execute one family plus selected variants and collect normalized comparison results

Key outcome:
- benchmark charts and summaries consume a stable multi-run result model

Status:
- completed on 2026-03-07
- implementation now lives in `src/lib/benchmarkRunModel.ts` plus the benchmark-runner path in `src/lib/stores/simulationStore.svelte.ts`
- benchmark mode can now run the selected base family alone or sweep the generated family variants serially through the existing worker
- normalized results now capture breakthrough PVI/time, water-cut series, pressure series, recovery series, and resolved reference-comparison summaries per run

### B5. Explicit reference policy

Goal:
- define what counts as “reference” per benchmark family

Policy:
- homogeneous BL: analytical Buckley-Leverett reference
- heterogeneous BL: refined numerical reference
- depletion: depletion analytical reference where applicable

Key outcome:
- the UI no longer implies analytical equivalence where that claim is no longer valid

Status:
- completed on 2026-03-07
- benchmark run results now expose `referencePolicy`, `referenceComparison`, and scenario-specific comparison outputs in `src/lib/benchmarkRunModel.ts`
- benchmark summary cards now show reference label, policy summary, and scenario-appropriate diagnostics for analytical BL, numerical BL, and depletion cases
- heterogeneous BL benchmark summaries no longer imply that analytical overlay remains the primary truth metric

### B6. Benchmark-specific charts and defaults

Goal:
- replace one-size-fits-all chart defaults with benchmark-family-specific panel composition

Planned defaults:
- BL: water cut vs PVI with breakthrough markers, recovery/cumulative oil vs PVI, separate pressure diagnostics
- depletion: oil rate, cumulative oil/recovery, pressure/decline diagnostics

Styling policy:
- color identifies case/variant
- line style identifies quantity or reference type
- pressure and water-cut-first reading stay separate by default

Status:
- completed on 2026-03-07
- benchmark chart layout now uses a shared typed contract in `src/lib/charts/rateChartLayoutConfig.ts`
- benchmark mode now derives chart defaults from family metadata plus explicit reference policy in `src/lib/charts/benchmarkChartConfig.ts`
- BL benchmark charts now default to breakthrough/recovery/pressure panels with benchmark-aware x-axis options instead of the generic rates/cumulative/diagnostics bundle
- depletion benchmark charts now default to oil-rate, cumulative/recovery, and pressure panels without surfacing irrelevant waterflood curves by default
- the reusable chart sub-panel now resets curve visibility when benchmark layouts swap curve sets, preventing stale toggle state across panel remaps

### B7. Multi-run benchmark overlays

Goal:
- extend benchmark charts from benchmark-specific single-run defaults to readable multi-run comparison overlays

Key outcome:
- benchmark results can be compared visually across a base run, axis variants, and reference traces without overloading one generic single-run chart contract

Status:
- completed on 2026-03-07
- added `src/lib/charts/BenchmarkChart.svelte` as a benchmark-specific comparison container layered on top of the shared `ChartSubPanel.svelte` primitive
- added `src/lib/charts/benchmarkComparisonModel.ts` to translate stored benchmark results into base-plus-variant overlay panels and family-appropriate reference traces
- `src/App.svelte` now switches benchmark mode to the comparison chart when stored results exist for the active benchmark family instead of reusing the live single-run runtime arrays
- BL benchmark overlays now compare water cut, recovery, and pressure across stored runs while carrying a shared analytical reference trace where applicable
- depletion benchmark overlays now compare oil rate, recovery/cumulative behavior, and pressure against an analytical depletion reference trace
- focused regression coverage now exercises the overlay model in `src/lib/charts/benchmarkComparisonModel.test.ts`

### B8. Benchmark UI workflow

Goal:
- expose family selection, enabled sensitivities, and comparison/reference behavior without collapsing benchmark mode into the generic scenario builder

Key outcome:
- benchmark mode becomes a comparison tool rather than only a preset launcher

### B9. Regression coverage and docs

Goal:
- lock benchmark semantics, sensitivity generation, reference policy, and chart defaults with tests and synchronized docs

Key outcome:
- future benchmark edits cannot drift silently

## Risks / Design Constraints

- Pressure-controlled BL benchmarks are more correct for Rust parity but less immediately intuitive than fixed-rate storytelling.
- Exact benchmark semantics and user-facing display horizon should remain independent decisions.
- Heterogeneous BL sensitivity should not be presented as a strict analytical-comparison benchmark.
- Multi-case overlays will become unreadable if color encodes quantity instead of case; quantity should move to panel structure and line style.
- Benchmark mode may need a dedicated chart container rather than forcing all needs through the current single-run chart contract.

## Next Action After Review

- Next active implementation slice is `B8`:
  - refine benchmark execution selection now that stored multi-run overlays exist
  - keep benchmark mode clearly distinct from the generic scenario builder while reducing inline action-button sprawl
  - preserve benchmark-to-custom cloning and explicit reference semantics while revisiting the deferred table-style selector idea
