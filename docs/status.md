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
- The remaining benchmark mismatch is reference-policy and presentation-related, not runner-ownership-related:
  - BL base families and their generated variants now share one registry plus one execution/result contract
  - the next step is making reference meaning explicit in the runner/UI for analytical versus numerical comparisons
- The next workstream is benchmark modernization: explicit reference handling, benchmark-specific charts, and multi-case overlay plumbing.

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

### B7. Benchmark UI workflow

Goal:
- expose family selection, enabled sensitivities, and comparison/reference behavior without collapsing benchmark mode into the generic scenario builder

Key outcome:
- benchmark mode becomes a comparison tool rather than only a preset launcher

### B8. Regression coverage and docs

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

- Next active implementation slice is `B5`:
  - make analytical-versus-numerical reference handling explicit in the runner/UI output contract
  - keep the new multi-run result model stable while refining what each family means by “reference”
  - only then wire benchmark-specific chart restructuring and multi-case overlays on top of that explicit comparison contract
