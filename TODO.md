# TODO — ResSim Frontend Execution Plan

This file is the live execution tracker for the next frontend workstream. It replaces the benchmark-only backlog now that benchmark modernization is complete and the main remaining work is product/workflow coherence.

## Current Objective (2026-03-07)

Make ResSim feel like one deliberate scientific application instead of several adjacent tools. The next workstream is about workflow clarity, warning consistency, benchmark discoverability, compact inputs, unified outputs, and a more disciplined visual design.

Primary review source:

- `docs/FRONTEND_UI_AUDIT_2026-03-07.md`

## Resume State

- Status: review complete, execution plan rewritten, implementation not started.
- Next slice: `F1` product workflow and information architecture.
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
