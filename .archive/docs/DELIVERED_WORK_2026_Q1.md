# Delivered Work Through 2026 Q1

This file keeps the important delivered work that used to clutter `TODO.md`. It is intentionally summary-level: enough to preserve context, not enough to become a second backlog.

## Scenario And Catalog Refactor

- Replaced the older case-library-heavy navigation with a scenario-first workflow centered on `scenarios.ts` and `ScenarioPicker.svelte`.
- Split canonical scenarios into per-scenario files under `src/lib/catalog/scenarios/`.
- Added `ScenarioCapabilities` and analytical-output contracts so chart defaults, sweep geometry, and routing behavior are scenario-owned.
- Landed sensitivity-level analytical overlay metadata and scenario-owned chart layouts.

## Sweep And Comparison Work

- Implemented Craig areal, Dykstra-Parsons vertical, and Stiles-style combined sweep interpretation.
- Made sweep-panel visibility and geometry scenario-driven instead of inferred ad hoc from variant inputs.
- Fixed preview and pending overlay behavior for sweep comparison charts.
- Reduced redundant snapshot density and improved color stability for multi-run comparison workflows.

## Depletion And Analytical Contract Fixes

- Fixed the Dietz well-location analytical gap so producer location now affects the depletion analytical helper where it should.
- Added analytical-contract tests that verify scenario dimensions marked `affectsAnalytical: true` genuinely perturb the analytical result.
- Added Arps decline and Havlena-Odeh material-balance diagnostics to depletion studies.

## Gas, Three-Phase, And Black-Oil Delivery

- Added gas-oil Buckley-Leverett support for `gas_injection`.
- Added `gas_drive` plus gas diagnostic surfaces such as p/z style interpretation and producing GOR.
- Corrected the gas-oil capillary-pressure convention and added `s_org` to the three-phase path.
- Added explicit gas cumulative material-balance reporting in three-phase mode.
- Added black-oil PVT generation, bubble-point tracking, phase-split logic, pressure-dependent mobility, and UI support for black-oil inputs.

## UI And Workflow Cleanup

- Reworked custom mode into grouped, denser scientific input sections.
- Added preset starting points and validation warnings for implausible or expensive setups.
- Improved run controls, warning handling, chart legends, and 3D default behavior for waterflood and gas contexts.

## Reference Notes Worth Preserving

- The remaining legacy benchmark files are not dead code yet. They still carry active production dependencies.
- `sweep_ladder` intentionally uses shared analytical overlays for teaching clarity.
- The black-oil pressure solve uses a saturated-region `c_o` fallback by design.
