# ResSim Roadmap

This roadmap is future-facing. Completed work has been moved out of `TODO.md` into `docs/DELIVERED_WORK_2026_Q1.md` so the active plan stays readable.

## Prioritization Principles

The ordering below follows standard reservoir-engineering practice and the literature already referenced in the project.

1. Validate before expanding. Comparative-solution and benchmark evidence should lead black-oil and three-phase growth.
2. Keep analytical methods honest about assumptions. Buckley-Leverett, Craig, Dykstra-Parsons, Stiles, Dietz, Fetkovich, Arps, and Havlena-Odeh all have narrow validity ranges.
3. Reduce architectural duplication before adding new UI surfaces. The remaining benchmark layer and output-selection plumbing are still more expensive than they should be.
4. Add new physics only after the existing interpretation and diagnostics are trustworthy.

## Priority 1: Scientific Validation And Closure

### 1.1 Black-oil validation

- Add SPE-style comparative-solution coverage for black-oil cases before extending into more gas-cap and compositional features.
- Add grid-convergence checks for pressure, Rs, Bo, and liberated-gas behavior in volatile-oil style depletion.
- Document current black-oil solver safeguards in user-facing physics notes, especially the saturated-region `c_o` fallback used to keep the IMPES pressure solve stable.

Why first:
- In the reservoir-simulation literature, black-oil extensions are only meaningful when the pressure equation, PVT coupling, and material-balance behavior are benchmarked against accepted reference problems.

### 1.2 Three-phase validation

- Define the bar for leaving `experimental` status.
- Add gas-injection and gas-drive acceptance tests that check breakthrough timing, gas saturation evolution, and phase-closure diagnostics.
- Clarify what is and is not reported in current material-balance diagnostics: water and gas are explicit, oil is residual.

Why first:
- The code now contains more three-phase capability than the docs claim, but validation still lags implementation.

### 1.3 Regression coverage gaps

- Add missing comparison-model tests for preview-only cases, per-variant depletion analytics, and color-index stability.
- Add a regression guard for the duplicated undersaturated `c_o = 1e-5 /bar` assumption shared by `physics/pvt.ts` and `analytical/materialBalance.ts`.

## Priority 2: Analytical Method Integrity

### 2.1 Enforce one analytical method per scenario

- Promote sweep to a first-class analytical family in scenario capabilities instead of partially piggybacking on Buckley-Leverett semantics.
- Make invalid primary-rate and overlay combinations impossible at the type / config level, not just test-detected.
- Route benchmark disclosure and comparison metadata through the same analytical-method contract.

Why next:
- This removes a class of ambiguous chart and policy behavior before more analytical methods are added.

### 2.2 Finish the sweep-method framework

- Generalize the current `sweep_combined` Stiles / Dykstra-Parsons toggle so other sweep scenarios can opt into multiple analytical methods without custom wiring.
- Keep the semantics explicit: total recovery comparison can improve while decomposition panels remain teaching diagnostics.
- Document the `sweep_areal` quarter-five-spot interpretation so users do not mistake the outer no-flow boundaries for a gridding bug.

Why next:
- Stiles is the right direction for layered floods, but the current implementation is still scenario-specific rather than framework-level.

## Priority 3: Scenario And Benchmark Architecture Consolidation

### 3.1 Remove the remaining split brain

- Collapse the remaining legacy benchmark layer into the scenario system where practical.
- Reduce dependence on `benchmarkCases.ts`, `caseCatalog.ts`, `ReferenceExecutionCard`, and the older benchmark-family adapters.
- Move shared run/result types into a clearer reference-run model owned by the scenario architecture.

### 3.2 Extract the output-selection view model

- Pull the active output payload selection out of `App.svelte` so charts, 3D view, and analytical helpers consume the same typed result.
- Use that refactor to simplify comparison-state wiring and reduce chart duplication.

Why this block matters:
- The largest remaining maintenance cost in the UI is architectural overlap, not missing widgets.

## Priority 4: Product Workflow And Data Portability

### 4.1 Per-scenario overrides

- Introduce scenario-preserving parameter overrides instead of forcing users into an all-or-nothing custom mode.
- Track per-field provenance and reset behavior.

### 4.2 Multi-case inspection

- Add multi-case 3D inspection and synchronized case selection across charts, summaries, and spatial views.
- Restore or explicitly retire the dormant saturation-profile path as part of the same output review.

### 4.3 Export and persistence

- Add JSON export/import for scenarios and custom studies.
- Add CSV/JSON result export for sensitivity runs and benchmark summaries.

Why after architecture cleanup:
- Persistence and comparison UX are much easier to implement once the output-selection model is unified.

## Priority 5: Physics Extensions After Validation

### 5.1 Gas-cap and depletion extensions

- Primary gas-cap scenario with gas-cap ratio `m` and material-balance framing.
- Gas-cap expansion and secondary gas-cap interpretation tied to the existing black-oil machinery.
- Gas-cap blowdown with p/z style diagnostics.

### 5.2 Vertical and areal sweep upgrades

- Kv/Kh-aware Warren-Root style blending between Dykstra-Parsons and perfect communication.
- Additional well-pattern correlations only after current sweep semantics are clean.

### 5.3 Longer-range reservoir-model features

- Aquifer models.
- Well schedules.
- Non-uniform grids and local refinement.
- Horizontal or deviated wells.

Why later:
- These features add breadth, but they should come after the simulator's current black-oil, gas, and analytical foundations are better validated.

## References Behind The Ordering

The roadmap direction is consistent with the classic references already used by the project and standard simulator-development practice:

- Buckley and Leverett, Welge: use analytical flood theory only where assumptions remain explicit.
- Craig; Dykstra and Parsons; Stiles: sweep methods are pattern- and communication-dependent, so method selection must stay explicit.
- Dietz; Fetkovich; Arps; Havlena and Odeh: depletion diagnostics are useful only when geometry, PVT, and drive assumptions are clear.
- SPE comparative-solution practice: benchmark the physics before claiming maturity for a simulator mode.

## Delivered Work

Recent delivered work lives in `docs/DELIVERED_WORK_2026_Q1.md`.
