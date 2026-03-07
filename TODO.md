# TODO — ResSim Active Work Plan

This file is the live plan for the next major workstream. Legacy Phase 1 / Phase 2 recovery notes and completed execution logs were removed from this tracker to keep it interruption-safe and reviewable.

## Proposed Direction (2026-03-07, pending review)

- [x] **Single benchmark source of truth** — benchmark physics definitions must exist in one logical place only; UI selectors, tests, and benchmark execution must resolve from that single source and never duplicate parameter payloads.
- [x] **Benchmark mode becomes a benchmark-family runner** — benchmark mode should support a base benchmark plus meaningful sensitivity variants, not only one static case launch.
- [x] **Buckley-Leverett base semantics match Rust** — BL benchmark families should use the exact validated Rust benchmark semantics as the physical reference point.
- [x] **Buckley-Leverett default x-axis is PVI** — benchmark charts for BL should default to pore volumes injected rather than time.
- [x] **Benchmark charts are benchmark-specific** — chart content should depend on benchmark family and reference type, not reuse the same generic curve set blindly.
- [x] **Depletion charts stay depletion-focused** — depletion benchmark views should emphasize oil rate, cumulative oil, and pressure/decline diagnostics, without default injection/water-cut clutter.
- [x] **Sensitivity axes must be meaningful** — BL benchmark sensitivities should initially support grid refinement, timestep refinement, and curated heterogeneity variants only where the comparison remains interpretable.
- [x] **Reference policy is explicit** — homogeneous BL variants compare against analytical Buckley-Leverett reference; heterogeneous BL variants compare against a refined numerical reference, not directly against analytical equality.

## Active Slice

- [x] **B0. Plan review and objective sign-off** — user approved proceeding with implementation starting from B1.
- [ ] **B2. Align BL benchmark base cases to exact Rust semantics (next)** — replace remaining rate-controlled BL benchmark semantics with exact Rust benchmark parity.

## Detailed Implementation Plan

- [x] **B1. Define benchmark-family schema and ownership**
  - Introduce a single benchmark registry contract that separates:
    - base physical definition
    - derived sensitivity definitions
    - reference policy
    - display defaults
    - run policy
  - Required metadata for each benchmark family:
    - `familyKey`, `label`, `description`
    - `scenarioClass` (`buckley-leverett`, `depletion`, later others)
    - `baseCase`
    - `sensitivityAxes`
    - `referenceKind` (`analytical`, `numerical-refined`)
    - `referenceCaseKey` or `referenceGenerator`
    - `defaultXAxis`
    - `defaultPanels`
    - `stylePolicy`
    - `runPolicy` (`single`, `sweep`, `compare-to-reference`)
  - Outcome:
    - implemented a benchmark-family registry layered over source benchmark case files under `src/lib/catalog/benchmark-case-data/`
    - selector options, family metadata, and runtime benchmark entries now resolve from the same registry contract
    - benchmark payloads now resolve from `src`-owned source files without duplicating parameter blobs in the registry

- [ ] **B2. Align BL benchmark base cases to exact Rust semantics**
  - Repoint frontend BL benchmark families to the same physical setup used by the validated Rust builders.
  - Keep physical semantics separate from user-facing run horizon.
  - Replace any remaining rate-controlled BL benchmark semantics with pressure-controlled Rust parity for benchmark families.
  - Record breakthrough criterion and accepted comparison metric in the benchmark metadata.
  - Acceptance:
    - BL base family matches the Rust benchmark definitions field-for-field where physically relevant
    - frontend benchmark runtime can be described as the same experiment as the Rust benchmark

- [ ] **B3. Introduce generated sensitivity variants without duplicating full cases**
  - Do not clone full JSON payloads for every benchmark variant.
  - Represent sensitivity variants as deltas from a benchmark family base case.
  - First supported BL axes:
    - grid refinement
    - timestep refinement
    - curated heterogeneity variants
  - For each axis, define:
    - allowed variant set
    - comparison meaning
    - whether analytical comparison remains valid
  - Acceptance:
    - one base family definition can generate the initial BL sensitivity suite
    - no N-way duplicated case payloads for each variant

- [ ] **B4. Build benchmark execution/result model for multi-run comparison**
  - Add a benchmark-runner path that can execute one benchmark family plus selected variants serially in the worker/store.
  - Define a normalized result model per run:
    - `caseKey`
    - `familyKey`
    - `variantKey`
    - `variantLabel`
    - `rateHistory`
    - `breakthroughPvi`
    - `breakthroughTime`
    - `watercutSeries`
    - `pressureSeries`
    - `recoverySeries`
    - `referenceComparison`
  - Include progress and cancellation for multi-run benchmark sweeps.
  - Acceptance:
    - benchmark mode can run base-only or base-plus-variants deterministically
    - results can be consumed by charts without special-case ad hoc plumbing

- [ ] **B5. Make benchmark reference handling explicit**
  - For homogeneous BL families:
    - preserve analytical Buckley-Leverett overlay and benchmark breakthrough comparison
  - For heterogeneous BL families:
    - compare against a refined numerical reference run
    - clearly label that analytical overlay is no longer the primary truth metric
  - For depletion families:
    - keep depletion analytical comparison where applicable
  - Add benchmark-level comparison outputs:
    - breakthrough shift
    - recovery difference at selected PVI/time landmarks
    - error summary against the applicable reference
  - Acceptance:
    - each benchmark family declares what “reference” means
    - benchmark charts and summaries do not imply analytical validity where it no longer applies

- [ ] **B6. Redesign benchmark chart composition and defaults**
  - Introduce benchmark-family-specific panel layouts instead of one generic chart bundle.
  - Proposed BL default panels:
    - water cut vs PVI with breakthrough markers
    - recovery / cumulative oil vs PVI
    - pressure diagnostics in a separate panel
    - optional rate panel only when it adds diagnostic value
  - Proposed depletion default panels:
    - oil rate vs time or dimensionless time
    - cumulative oil / recovery
    - average pressure / decline diagnostics
  - Curve-style policy:
    - color identifies case/variant
    - line style identifies quantity or reference type
    - avoid mixing pressure and water cut in one primary panel by default
  - Acceptance:
    - BL charts default to breakthrough-centric reading
    - depletion charts do not surface irrelevant waterflood curves by default
    - multi-case benchmark overlays remain readable without ambiguous legends

- [ ] **B7. Update chart/data plumbing for multi-case overlays**
  - Refactor chart inputs so benchmark views can render multiple runs and one reference together.
  - Decide whether to extend the current rate chart stack or add a benchmark-specific chart container on top of shared lower-level chart primitives.
  - Keep x-axis policy benchmark-aware:
    - BL default `PVI`
    - depletion default `time` or `tD` where meaningful
  - Add benchmark legend/group controls if needed for toggling cases and quantities.
  - Acceptance:
    - chart plumbing is no longer hard-wired to one simulation run + one analytical series only

- [ ] **B8. Update benchmark UI workflow**
  - Benchmark mode should let the user select:
    - benchmark family
    - enabled sensitivity axes / variants
    - comparison/reference mode when applicable
  - Keep benchmark mode distinct from the faceted scenario builder mental model.
  - Preserve one-click clone into custom scenario from any selected base benchmark or variant.
  - Acceptance:
    - benchmark mode is clearly for verification/comparison
    - custom scenario mode remains clearly for ad hoc modeling

- [ ] **B9. Add regression tests for parity, sweeps, and chart defaults**
  - Add tests for:
    - benchmark family registry integrity
    - Rust/frontend BL semantic parity
    - variant generation without payload duplication
    - reference-policy gating for heterogeneous families
    - chart default x-axis/panel selection by benchmark family
  - Acceptance:
    - future benchmark edits cannot silently drift from the benchmark contract

- [ ] **B10. Refresh docs and benchmark guidance**
  - Update benchmark docs to explain:
    - exact benchmark semantics
    - sensitivity meaning
    - default x-axis and panel rationale
    - analytical vs numerical reference policy
  - Keep `TODO.md` and `docs/status.md` synchronized after each implementation slice.
  - Acceptance:
    - user-facing and developer-facing docs describe the same benchmark system

## Important Design Notes To Keep In Scope

- [ ] **Do not let benchmark location drive architecture** — the correct home is whichever place can own the single source cleanly; duplication is the real problem, not whether the file sits in `public/cases` or near the benchmark registry module.
- [ ] **Do not equate exact physics parity with long UI horizon** — the benchmark physics definition and the user-facing display horizon should be separate knobs.
- [ ] **Do not compare heterogeneous BL directly to analytical as if it were still a strict reference** — heterogeneous variants need a refined numerical reference path.
- [ ] **Do not overload one chart with unrelated units just to save space** — pressure should be separated from water-cut-first breakthrough reading unless a specific combined diagnostic panel proves clearer.
- [ ] **Do not duplicate variants as copy-pasted cases** — sensitivity definitions should be generated from base-family metadata and deltas.

## Acceptance Criteria For The Whole Workstream

- [ ] There is exactly one benchmark definition source for each benchmark family.
- [ ] Frontend BL benchmark families represent the same physical experiment as the validated Rust BL benchmarks.
- [ ] Benchmark mode can run base case plus meaningful sensitivities without duplicating full case payloads.
- [ ] BL charts default to breakthrough-centric panels and `PVI` x-axis.
- [ ] Depletion charts default to depletion-relevant curves only.
- [ ] Heterogeneous BL comparisons use explicit numerical-reference policy.
- [ ] Benchmark legends, colors, and line styles remain readable for multi-case overlays.
- [ ] Clone-to-custom provenance still works from benchmark mode.
- [ ] Tests protect benchmark semantics, sweep generation, and chart defaults.

## Deferred / Separate Backlog

- [ ] Keep this section reserved for future cross-workstream items that are not part of the current benchmark-modernization objective.

## Retained Long-Term Options

These are intentionally kept as non-active options. They remain good future directions, but they should not compete with the current benchmark-modernization workstream.

### Important Later

These still fit the product direction well and are likely to matter after the benchmark system stabilizes.

#### Simulation and physics expansion

- [ ] Well schedule support — time-varying BHP/rate changes and workover-style control changes.
- [ ] Three-phase flow — phased oil/water/gas extension once the benchmark framework is stable.
- [ ] Aquifer boundary conditions — Carter-Tracy or Fetkovich-style influx support.
- [ ] Per-cell or per-layer porosity variation.
- [ ] Per-cell initial water saturation / transition-zone initialization.
- [ ] Additional published benchmark families beyond BL/depletion.

#### Benchmark and comparison tooling

- [ ] Grid-convergence study preset family.
- [ ] A/B run comparison overlays.
- [ ] Relative error (%) diagnostic curves.
- [ ] Uncertainty and sensitivity batch runner beyond curated benchmark sensitivities.

#### Visualization and charting

- [ ] Sw profile plot evolution and tighter integration with benchmark mode.
- [ ] Cross-section / slice viewer for i/j/k inspection in the 3D view.
- [ ] Summary statistics panel for OOIP, pore volume, RF, average pressure, average saturation, water cut, and VRR.

#### Scenario and reporting workflow

- [ ] Structured scenario export/import.
- [ ] CSV/JSON export of results and benchmark summaries.

#### Wells and advanced reservoir modeling

- [ ] Multi-well patterns such as 5-spot, line-drive, and custom placements.
- [ ] Non-uniform cell sizes and local grid refinement.

#### Analytical and diagnostic expansion

- [ ] Areal sweep efficiency charting.
- [ ] Depletion analytical calibration against additional published references.

### Nice To Have Only

These are still reasonable ideas, but they are less central to the current product direction and should stay behind the important-later group.

#### Tooling and process

- [ ] Benchmark trend tracking across commits or CI runs.

#### Visualization and comparison UX

- [ ] Comparative visualization mode for side-by-side scenarios or delta views.
- [ ] Multi-chart synchronized zoom/pan.
- [ ] Responsive/mobile chart and 3D layout improvements.
- [ ] Phase relative permeability / capillary curve visualization.

#### Scenario workflow and reporting

- [ ] Report export for plots and key metrics.
- [ ] Undo/redo for parameter changes.

#### Advanced modeling extensions

- [ ] Horizontal or deviated well model with generalized Peaceman PI.
- [ ] Per-cell capillary pressure variation and capillary hysteresis.

#### Analytical overlay expansion

- [ ] Fetkovich type-curve overlay expansion.

### Dropped From Retained Options

These were removed from the retained list because they were duplicates or too vague to justify separate tracking right now.

- [x] Make the Sw profile a more explicit companion to the spatial 3D view — folded into Sw profile integration work.
- [x] Three-phase flow and broader physics extensions — replaced by the more specific three-phase flow item.
- [x] CSV/JSON export of results — folded into the more specific scenario/results export item.
- [x] General scenario import/export — replaced by structured scenario export/import.
- [x] Cross-section / slice viewer improvements — replaced by the more specific slice-viewer item.
- [x] Additional published benchmark families beyond the current BL/depletion scope — retained once, in the physics/benchmark expansion group.
