# TODO — ResSim Frontend Execution Plan

This file is the live execution tracker for the next frontend workstream. It replaces the benchmark-only backlog now that benchmark modernization is complete and the main remaining work is product/workflow coherence.

## Current Objective (2026-03-07)

Make ResSim feel like one deliberate scientific application instead of several adjacent tools. The next workstream is about workflow clarity, warning consistency, benchmark discoverability, compact inputs, unified outputs, and a more disciplined visual design.

Primary review source:

- `docs/FRONTEND_UI_AUDIT_2026-03-07.md`

## Resume State

- Status: `F1.4`, `F1.5`, and `F1.7` are complete and validated; the family-local `Case Library` owns reference entry, `Type Curves` stays selected for Fetkovich-driven state, and the remaining benchmark-named inputs wrapper has been absorbed into `ModePanel`.
- Next slice: move into `F1.9` cleanup for the remaining benchmark-mode-only plumbing, tests, and docs phrasing.
- Reviewed F1 direction:
  - explicit page regions: `Inputs`, `Run`, `Outputs`
  - `Outputs` owns comparison from day one
  - reviewed family direction: `Waterflood`, `Depletion Analysis`, `Type Curves`, `Scenario Builder`
- After `F1`: `F2` warning/label normalization and `F3` benchmark disclosure should proceed before layout polish.
- Constraint: preserve completed benchmark-modernization behavior unless `F1` explicitly decides to reposition benchmark mode in the product.

## Active Workstream

- [ ] **F1. Decide the product workflow and information architecture**
  - Replace the current benchmark top-level mode with explicit product families:
    - `Waterflood`
    - `Depletion Analysis`
    - `Type Curves`
    - `Scenario Builder`
  - Keep family labels short in navigation and use explanatory subtitles inside the `Inputs` region.
  - Define the canonical page structure:
    - inputs
    - run + warnings
    - outputs
  - `Outputs` must own comparison and sensitivity review from day one rather than using a separate comparison/benchmark destination.
  - Move current benchmark/reference families into their owning product families:
    - Buckley-Leverett reference families into `Waterflood`
    - Dietz-style fully analytical depletion references into `Depletion Analysis`
    - Fetkovich into `Type Curves` as the seed case for future type-curve workflows
  - Decide the input source model.
    - Current recommendation: replace separate `Presets` and `Reference Cases` with one library-style source plus `Custom`
    - Reason: many curated cases are not true literature references, so calling everything a reference risks weakening trust
    - Recommended library grouping inside the source selector:
      - literature references
      - internal reference / validation cases
      - curated starting cases
    - If `Reference Cases` is retained as the source label, only true reference cases should live there
  - Replace `Clone to Custom` with an in-family `Customize` action:
    - when a library/reference case is active, ordinary edits stay locked
    - only allowed sensitivity toggles remain available
    - pressing `Customize` switches the same family into `Custom` while carrying provenance and seeded parameters
  - Add a concrete reference-case narrative for users:
    - where the case comes from
    - what settings are fixed
    - what sensitivities are allowed
    - what reference policy applies
    - when to stay in the library flow versus when to switch to custom
  - Prevent `Scenario Builder` from becoming a junk drawer:
    - define explicit entry criteria for what belongs there
    - explain in the UI why a case is routed there when it is not supported inside `Waterflood`, `Depletion Analysis`, or `Type Curves`
  - Acceptance:
    - a user can tell where to build scenarios, where to validate them, and where to inspect results without guessing
    - the workflow no longer depends on hidden mental-model jumps between tabs/modes
    - the app no longer needs a separate benchmark top-level mode to express verification workflows
    - the source model uses truthful names for literature references versus curated internal cases

### F1 Implementation Plan

These slices are ordered for safe migration. The goal is to change architecture once, keep benchmark-result logic intact, and avoid half-migrated shell states.

#### F1.0 Lock Architecture Decisions

- [ ] Finalize the F1 product contract before code changes:
  - top-level families: `Waterflood`, `Depletion Analysis`, `Type Curves`, `Scenario Builder`
  - page regions: `Inputs`, `Run`, `Outputs`
  - `Outputs` owns comparison from day one
  - source model recommendation: `Case Library` + `Custom`
  - library grouping inside `Case Library`:
    - literature references
    - internal reference / validation cases
    - curated starters
  - `Customize` replaces `Clone to Custom`
  - locked/reference cases stay read-only except for approved sensitivity controls
- Primary files to update after sign-off:
  - `TODO.md`
  - `README.md`
  - `docs/FRONTEND_UI_AUDIT_2026-03-07.md`
- Acceptance:
  - no unresolved naming or workflow ambiguity remains before store/UI migration begins

#### F1.1 Introduce New Navigation And Source Contracts

- [x] Add explicit navigation/source state contracts before changing components.
- New store concepts:
  - `activeFamily`
  - `activeSource`
  - `activeLibraryCaseKey`
  - `activeLibraryGroup`
  - `activeComparisonSelection`
  - `editabilityPolicy`
- Replace the current overloaded `activeMode` / benchmark-only flow gradually rather than all at once.
- Keep temporary compatibility shims while the shell migrates.
- Primary files:
  - `src/lib/ui/modePanelTypes.ts`
  - `src/lib/stores/phase2PresetContract.ts`
  - `src/lib/stores/simulationStore.svelte.ts`
- Acceptance:
  - the store can represent family, source, library case, and custom state independently
  - no component needs `benchmark` as a top-level mode to understand reference workflows

#### F1.2 Build A Unified Case-Library Catalog Layer

- [~] Introduce a case-library adapter instead of exposing separate preset and benchmark ownership to the UI shell.
- Current progress inside `F1.2`:
  - unified `caseLibrary` adapter exists and is re-exported through `caseCatalog`
  - Buckley-Leverett refined families are now classified as `internal-reference`
  - Dietz and Fetkovich references remain classified as `literature-reference`
  - adapter entries now carry richer provenance text alongside short source labels and reference-source labels
  - current store selection state now threads `group`, `sourceLabel`, `referenceSourceLabel`, and `provenanceSummary` from resolved library entries where available
  - non-benchmark selection now resolves exact curated preset matches via case-library metadata instead of treating facet keys as library ids
- Remaining `F1.2` tasks before shell migration:
  - decide which curated starters remain in the library versus being dropped or reworded
  - remove remaining app/panel assumptions that benchmark/reference cases only live behind the legacy benchmark tab
  - replace the temporary depletion-tab hosting of type-curve references with dedicated family navigation in `F1.4` / `F1.5`
  - consolidate the now-duplicated benchmark-panel and family-local library-selector reference entry surfaces during `F1.4`
- The adapter should normalize:
  - family ownership
  - library group (`literature-reference`, `internal-reference`, `curated-starter`)
  - editability policy
  - citation/source label
  - reference policy summary
  - sensitivity availability
- Re-home current content:
  - BL benchmark families -> `Waterflood`
  - Dietz reference families -> `Depletion Analysis`
  - Fetkovich -> `Type Curves`
  - exploratory presets -> `Scenario Builder`
  - decide explicitly which existing preset entries survive as curated starters versus being dropped
- Primary files:
  - `src/lib/catalog/caseCatalog.ts`
  - new case-library module under `src/lib/catalog/`
  - `src/lib/catalog/presetCases.ts`
  - `src/lib/catalog/benchmarkCases.ts`
  - `src/lib/catalog/caseCatalog.test.ts`
- Acceptance:
  - the UI can query one library API instead of deciding between presets and benchmarks itself
  - every library case carries a truthful provenance/source classification

#### F1.3 Generalize The Reference Runner In The Store

- [~] Remove benchmark-mode gating from execution logic while preserving the normalized benchmark result model.
- Current progress inside `F1.3`:
  - store runner and Customize gating now resolve reference capability from the active library entry instead of only from `activeMode === 'benchmark'`
  - store now exposes an explicit `activateLibraryEntry(...)` path so future family-local library selectors can activate BL, Dietz, and Fetkovich without depending on the benchmark tab
  - output/result selection now follows the active reference family rather than `activeMode === 'benchmark'`
  - shared navigation/base-profile editability now classifies family-owned literature/internal references as locked reference cases even outside the legacy benchmark tab
  - runtime warnings and empty-state messaging now describe the active reference runner/case flow instead of assuming the benchmark tab is the only entry path
  - store/runtime now expose reference-oriented aliases for active family, provenance, sweep state, run results, and execution actions so new callers no longer need benchmark-prefixed APIs
- New follow-up gap discovered during `F1.3`:
  - legacy benchmark-prefixed prop names remain in `ModePanel` / `BenchmarkPanel` and the benchmark chart module contract; defer renaming those UI-facing compatibility surfaces to `F1.4` / `F1.5` while the benchmark panel still exists
- Migrate current runner behavior from “benchmark mode only” to “reference-capable library cases inside owning families.”
- Preserve and reuse:
  - benchmark run specs
  - normalized comparison results
  - analytical/numerical reference policy logic
- Add store rules for:
  - read-only library/reference selection
  - `Customize` handoff into family-local custom state
  - family-aware result filtering in outputs
- Primary files:
  - `src/lib/stores/simulationStore.svelte.ts`
  - `src/lib/benchmarkRunModel.ts`
  - `src/lib/stores/simulationStorePolicyWiring.test.ts`
  - `src/lib/appStoreDomainWiring.test.ts`
- Acceptance:
  - BL, Dietz, and Fetkovich reference cases can execute without a benchmark top-level mode
  - `Customize` can seed custom state from a library case while preserving provenance

#### F1.4 Replace The Top-Level Shell

- [x] Replace the current mode-tab shell with a family-first `Inputs / Run / Outputs` shell.
- Remove benchmark as a top-level tab.
- Add family subtitles and source selectors in `Inputs`.
- Keep the current detailed section components alive during the first shell pass instead of redesigning them at the same time.
- Current progress inside `F1.4`:
  - `ModePanel` now exposes family-first navigation and no longer shows `Benchmarks` as a top-level tab
  - the case-library selector is now scoped to the active family instead of the old depletion-plus-type-curves bridge
  - reference-family cases now route through the existing benchmark panel inside their owning family instead of requiring a separate top-level destination
  - `App` now labels explicit `Inputs`, `Run`, and `Outputs` regions in the current shell
  - the legacy reference selector has been removed from `BenchmarkPanel`; the family-local `Case Library` now owns reference entry as the single inputs-side selector
- Remaining `F1.4` tasks:
  - none; the next cleanup continues in `F1.5`
- Primary files:
  - `src/App.svelte`
  - `src/lib/ui/modes/ModePanel.svelte`
  - `src/lib/ui/modePanelComposition.test.ts`
  - `src/lib/ui/modePanelFlows.test.ts`
- Acceptance:
  - navigation is family-first and source-aware
  - page regions are explicit even before compact-layout work starts

#### F1.5 Build The Inputs Region Around Case Library And Custom

- [x] Replace the old benchmark panel split with one Inputs-region workflow.
- Inputs-region responsibilities:
  - family selector
  - source selector (`Case Library` / `Custom`)
  - library group switch or grouped list
  - case disclosure panel with citation/source, fixed settings, sensitivities, and reference policy
  - `Customize` action
  - existing parameter sections, honoring editability policy
- Current progress inside `F1.5`:
  - the temporary source-status chip has been replaced with a real family-local `Case Library` / `Custom` selector in `ModePanel`
  - source switching now performs real actions: starter cases transition into custom editing, reference cases use the existing seeded-customize flow, and custom runs can restore back to a curated family case
  - the inputs-surface prop contract between `App`, `ModePanel`, and `BenchmarkPanel` now uses `reference*` naming instead of `benchmark*` naming
  - user-facing inputs copy now says `Seeded from` / `Customize` instead of `Clone to Custom` where the flow is library-to-custom seeding
  - the inputs region now includes a richer case disclosure block covering citation/source, fixed-settings behavior, allowed sensitivities, and reference policy for the active family case
  - grouped library sections remain the default browsing model; no dedicated group switch will be added at the current catalog size, and the next escalation path is lightweight group filter chips only if family libraries grow materially
  - the remaining benchmark-named inputs wrapper has been absorbed into `ModePanel`, leaving the family-local `Case Library` as the single inputs-side reference surface
- Remaining `F1.5` tasks:
  - none; the next cleanup continues in `F1.9`
- Locked behavior:
  - library/reference cases show fixed inputs as read-only
  - only approved sensitivity selectors stay editable
  - `Customize` unlocks the case by switching source to `Custom`
- Primary files:
  - `src/lib/ui/modes/ModePanel.svelte`
  - replace or retire `src/lib/ui/modes/BenchmarkPanel.svelte`
  - `src/lib/ui/sections/ScenarioSectionsPanel.svelte`
  - `src/lib/ui/modePanelTypes.ts`
- Acceptance:
  - the user can select, inspect, and customize a library case without leaving the owning family

#### F1.6 Rebuild The Run Region As The Canonical Execution Surface

- [x] Pull run controls, warnings, and “what will run” summary into one explicit Run region.
- Run region should show:
  - validation/runtime/reference warnings
  - run buttons and progress
  - run manifest describing the active family, source, case, and sensitivity selection
  - explicit reference/comparison policy summary for library/reference runs
- Current progress inside `F1.6`:
  - the `Run` region now includes a run manifest driven by active family/source/case metadata rather than only generic simulator controls
  - reference policy, allowed sensitivities, and seeded/custom provenance are now summarized beside the run controls instead of living only inside the inputs disclosure flow
  - reference sweep progress and sweep-specific errors now surface in the `Run` region rather than only inside the inputs-side reference panel
  - the execution-set selector itself now lives in a dedicated run-region reference execution card instead of the inputs-side reference panel
- Remaining `F1.6` tasks:
  - none; the next comparison-ownership move continues in `F1.7`
- Primary files:
  - `src/App.svelte`
  - `src/lib/ui/cards/RunControls.svelte`
  - `src/lib/ui/feedback/WarningPolicyPanel.svelte`
  - `src/lib/warningPolicy.ts`
- Acceptance:
  - the user can understand exactly what will execute and what it will be compared against before running

#### F1.7 Make Outputs The Single Home For Comparison

- [x] Reorganize outputs so comparison is no longer treated as a separate benchmark-mode concept.
- Outputs responsibilities:
  - compact result summary
  - case/run comparison selector
  - charts
  - 3D view
  - supporting diagnostics/profile views
- Keep the existing comparison model and benchmark chart plumbing where possible, but mount it under Outputs rather than benchmark mode.
- Current progress inside `F1.7`:
  - stored reference result cards have moved out of the inputs-side reference panel and into an outputs-side summary card beside the comparison charts
  - the inputs-side reference panel no longer owns stored comparison history, so `ModePanel` no longer needs the `referenceRunResults` compatibility prop thread
  - the outputs-side summary card now owns the active comparison-case focus and drives chart defaults through the existing `activeComparisonSelection` store contract
  - the saturation-profile surface now follows the selected comparison case when one is focused, using stored reference snapshots and case-specific rock/fluid settings instead of always showing only the live runtime state
  - the 3D view now follows the selected comparison case as well, including stored history playback, well overlays, and source labeling instead of always reflecting only the live runtime state
  - the remaining benchmark-named output chart/config/model surfaces have been renamed to reference-comparison equivalents, so outputs no longer present benchmark-specific naming at the chart shell level
- Primary files:
  - `src/App.svelte`
  - `src/lib/charts/RateChart.svelte`
  - `src/lib/charts/BenchmarkChart.svelte`
  - `src/lib/charts/benchmarkComparisonModel.ts`
  - `src/lib/visualization/3dview.svelte`
- Remaining `F1.7` tasks:
  - none; the next cleanup returns to `F1.4` / `F1.5`
- Acceptance:
  - outputs can render single-run, reference-vs-simulation, and sensitivity comparisons from one region

#### F1.8 Define And Enforce Scenario Builder Boundaries

- [ ] Explicitly define what belongs in `Scenario Builder`.
- Immediate requirement:
  - do not use it as a silent fallback for unsupported workflows
  - if a user is redirected there, explain why
- Later requirement:
  - reduce the number of cases that need that redirection at all
- Primary files:
  - `src/lib/catalog/catalog.json`
  - `src/lib/catalog/caseCatalog.ts`
  - `src/lib/ui/modes/ModePanel.svelte`
  - docs after F1 completes
- Acceptance:
  - `Scenario Builder` reads as intentional exploratory modeling, not as a catch-all bucket

#### F1.9 Remove Legacy Benchmark-Mode Plumbing And Refresh Tests

- [ ] After the new shell and store paths are stable, remove legacy benchmark-mode-only code and tests.
- Retire or repurpose:
  - benchmark top-level mode selectors
  - benchmark-only UI props/types
  - benchmark-only store guards
  - docs phrased around a separate benchmark mode
- Primary files:
  - `src/lib/ui/modes/ModePanel.svelte`
  - `src/lib/ui/modePanelTypes.ts`
  - `src/lib/stores/simulationStore.svelte.ts`
  - `README.md`
  - `docs/BENCHMARK_MODE_GUIDE.md`
  - test files under `src/lib/ui/`, `src/lib/catalog/`, and store wiring tests
- Acceptance:
  - no live code path depends on `benchmark` as a top-level mode
  - tests protect the new family/source/output architecture

### F1 Cutover Strategy

- [ ] Keep the current result-model and comparison math intact while migrating the shell.
- [ ] Migrate contracts first, then catalog/library, then store runner, then shell, then outputs, then cleanup.
- [ ] Do not combine F1 shell migration with F6 visual compaction in the same implementation slice.
- [ ] Do not rename every label in the same slice as the state-model migration; keep F2 separate.

### F1 Risks To Watch During Implementation

- [ ] `Type Curves` may be thin initially; the shell must frame it as an analytical family, not a fully broad product area.
- [ ] If `Case Library` is adopted, the UI must visually distinguish literature references from curated starters.
- [ ] Existing mode-based tests will break in large numbers unless migrated slice-by-slice.
- [ ] The 3D/output comparison experience must not regress while comparison ownership moves into `Outputs`.

- [ ] **F2. Unify warnings, labels, and terminology**
  - Keep one warning-policy data model, but redesign the UI around one canonical warning surface plus section-local status/error indicators.
  - Normalize vocabulary across the app:
    - mode names
    - run actions
    - benchmark actions
    - analytical/reference language
    - result summaries
  - Rename ambiguous labels such as `Analytical Model` if the control actually selects a reference/solution mode.
  - Standardize text sizing and hierarchy across section headers, field labels, badges, and microcopy.
  - Acceptance:
    - warnings read as one system instead of three related surfaces
    - labels describe user intent rather than implementation details

- [ ] **F3. Expose benchmark settings and reference policy clearly**
  - Add benchmark case disclosure cards or compact spec tables showing:
    - grid/model size
    - key fluid/rock settings
    - well controls
    - timestep/horizon
    - reference policy
    - variant deltas for sensitivities
  - Replace text-heavy stored benchmark result cards with a compact compare surface.
  - Make it obvious which outputs belong to the base case versus selected sensitivity variants.
  - Acceptance:
    - users can answer “what is this benchmark actually running?” before pressing run
    - stored results remain readable when several variants exist

- [ ] **F4. Unify chart and output architecture**
  - Remove duplicated top-level interaction/state scaffolding between `RateChart` and `BenchmarkChart` where practical.
  - Keep one interaction model for x-axis selection, panel expansion, legends, and output summaries across live runs and benchmark comparisons.
  - Add compact output-summary cards/lists that sit above or beside charts instead of burying comparison data in long text blocks.
  - Acceptance:
    - chart behavior feels consistent regardless of run type
    - future output features do not require parallel implementation in multiple chart shells

- [ ] **F5. Add multi-case comparison beyond charts**
  - Add case selection/switching for the 3D view when multiple benchmark or sensitivity runs exist.
  - Extend comparison awareness to other output surfaces where it adds value:
    - saturation profile
    - compact summary cards
    - key diagnostics
  - Synchronize the selected case across summary, chart, and 3D inspection where appropriate.
  - Acceptance:
    - sensitivity studies can be inspected spatially, not only in charts
    - output surfaces stay synchronized around one selected comparison case

- [ ] **F6. Compact the input layout and reduce page height**
  - Reduce default section padding and vertical spacing.
  - Convert overly tall input groups into compact flowing cards/subcards where possible.
  - Revisit table-heavy sections and keep tables only where they clearly outperform compact forms.
  - Tighten margins and whitespace across the shell without making the UI cramped.
  - Acceptance:
    - common scenario editing takes materially less scrolling on desktop
    - dense scientific inputs still remain legible and editable

- [ ] **F7. Redesign light/dark themes and remove visual noise**
  - Replace near-black dark surfaces and flat-white light surfaces with more deliberate working themes.
  - Remove or significantly soften the reservoir-layer page background treatment.
  - Improve panel contrast, content focus, and overall data-first visual balance.
  - Acceptance:
    - both themes feel designed for sustained technical use
    - decorative background treatment no longer competes with data surfaces

- [ ] **F8. Finish the faceted selection product surface**
  - Audit every depletion, waterflood, and simulation facet against actual supported user workflows.
  - For each currently surfaced option, choose one:
    - fully support it
    - hide it
    - present it with an explicit reason and path to the right mode/workflow
  - Make the mode split and facet constraints readable from the UI itself, not only from code rules.
  - Acceptance:
    - the catalog surface no longer looks broader than the product actually is
    - users understand why an option is unavailable and what to use instead

- [ ] **F9. Refresh README and docs after the UI pass**
  - Update README, benchmark guide, docs index, and status docs after F1-F8 land.
  - Ensure the docs describe the final workflow and terminology, not transitional states.
  - Acceptance:
    - product docs match the live UI and current mental model

## Whole-Workstream Acceptance Criteria

- [ ] Inputs, run/warnings, outputs, and comparison review are visibly separated and easy to navigate.
- [ ] Warning handling reads as one system and points users back to the right input region.
- [ ] Benchmark cases expose their settings and reference policy clearly.
- [ ] Sensitivity studies can be inspected in charts, summaries, and 3D views.
- [ ] Labels and terminology are consistent across the app.
- [ ] Input panels are materially more compact without losing readability.
- [ ] Light and dark themes both feel intentional and data-first.
- [ ] Faceted selection truthfully represents what each mode is meant to do.
- [ ] `Scenario Builder` is positioned as an intentional exploratory workflow, not as a dump zone for unsupported cases.

## Completed Context

- [x] Benchmark modernization `B1` through `B10` completed on 2026-03-07.
- [x] Benchmark registry, explicit reference policy, selected-variant sweeps, benchmark-specific chart defaults, and benchmark docs are now baseline capabilities.

## Deferred / Later

- [ ] Well schedule support.
- [ ] Three-phase flow.
- [ ] Aquifer boundary conditions.
- [ ] Per-cell or per-layer porosity variation.
- [ ] Per-cell initial water saturation / transition-zone initialization.
- [ ] Additional published benchmark families beyond Buckley-Leverett and depletion.

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
