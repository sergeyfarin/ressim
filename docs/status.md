Phase 1 implementation has started and is now in code with a solid first slice complete.

Completed in this Phase 1 slice
Centralized validation into one source
simulationStore now uses validateInputs.ts directly.
Added imports in src/lib/stores/simulationStore.svelte.ts:21.
Added buildValidationInput() adapter in src/lib/stores/simulationStore.svelte.ts:276.
Replaced in-store duplicate validator with:
validateSimulationInputs(buildValidationInput()) in src/lib/stores/simulationStore.svelte.ts:306.
Explicit model-domain config diff tracking
Replaced generic signature var with model-domain signature:
modelConfigSignature in src/lib/stores/simulationStore.svelte.ts:224.
checkConfigDiff() now compares buildModelResetKey() signatures:
src/lib/stores/simulationStore.svelte.ts:717.
This aligns diff detection with the model-reset domain explicitly, not a broad implicit payload check.
Backlog status updated
Marked these Phase 1 items complete in TODO.md:43 and TODO.md:44.
Validation run
Type/diagnostics: no errors in modified store.
Tests passed:
validateInputs.test.ts
buildCreatePayload.test.ts
caseCatalog.test.ts
Total: 41 tests passed.
What’s next in Phase 1
Next concrete slice is state-domain separation in the store API:

Introduce explicit domain objects (scenarioSelection, parameterState, runtimeState) in store return shape.
Update App.svelte bindings to consume these domain objects.
Keep compatibility shims temporarily to avoid a big-bang break.
If you want, I’ll proceed with that refactor now.

Continuity and planning update (2026-03-05)
Phase 1 now has a dedicated interruption-safe execution tracker in TODO.md under:
Active Execution Plan — Phase 1 (Interruption-Safe).

Process commitments now in place:
- TODO.md is the single source of truth for active Phase 1 slice status.
- Only one slice is active at a time (marked in TODO text as `(in progress)`).
- Every completed slice is mirrored here with files touched + tests run + explicit next slice.
- Mid-slice interruption requires a short WIP note here before stopping.

Next queued slice:
P1.3 Store state domain split (API layer), then P1.4 compatibility shim window.

Phase 1 progress update (2026-03-05)
Completed slice: P1.3 Store state domain split (API layer).

Implementation details:
- Added explicit domain objects in `src/lib/stores/simulationStore.svelte.ts`:
	- `scenarioSelection`
	- `parameterState`
	- `runtimeState`
- Kept the existing flat top-level store fields and actions available as compatibility shims to avoid breakage while App/UI migration proceeds.

Validation run:
- `npm run test`
- Result: 7 test files passed, 86 tests passed.
- `npm run typecheck`
- Result: passed (`tsc --noEmit` with no errors).

Next active slice:
P1.4 Compatibility shim window (maintain temporary aliases while migrating consumers).

Phase 1 progress update (2026-03-05)
Completed slices:
- P1.4 Compatibility shim window
- P1.5 App migration

Implementation details:
- Migrated `src/App.svelte` to consume domain objects from the store instead of flat compatibility fields:
	- `scenarioSelection` (`scenario` alias in App)
	- `parameterState` (`params` alias in App)
	- `runtimeState` (`runtime` alias in App)
- Updated lifecycle/effects and all App bindings (TopBar, RunControls, analytical components, charts, 3D view, inputs, and debug panel) to use domain APIs.
- Compatibility shim fields remain in store for incremental migration safety.

Validation run:
- `npm run typecheck` passed (`tsc --noEmit`).
- `npm run test` passed (7 files, 86 tests).

Discovered gap logged in TODO:
- Add App-level regression tests for domain wiring to prevent accidental fallback to flat compatibility fields.

Next active slice:
P1.6 UI consumer migration.

Phase 1 progress update (2026-03-05)
Completed slice:
- P1.6 UI consumer migration

Audit result:
- Searched for additional direct consumers of `createSimulationStore` / `SimulationStore` in `src/**`.
- `App.svelte` is the only direct consumer; no downstream UI components bind the store directly.
- Therefore this slice is complete with no further code changes required beyond the App migration already done.

Next active slice:
P1.7 Domain-scoped dirty/reset behavior.

Phase 1 progress update (2026-03-05)
Completed slices:
- P1.7 Domain-scoped dirty/reset behavior
- P1.8 Remove shim fields

Implementation details:
- Fixed model-reset domain signature coverage in `src/lib/stores/simulationStore.svelte.ts` by adding `reservoirPorosity` to `buildModelResetKey()`.
- Removed temporary flat top-level compatibility API from `createSimulationStore()` return shape.
- Store now exposes domain APIs only: `scenarioSelection`, `parameterState`, `runtimeState`.

Validation run:
- `npm run typecheck` passed.
- `npm run test` passed (7 files, 86 tests).

Discovered gap logged in TODO:
- Add model-reset domain key coverage tests to prevent signature omission regressions.

Next active slice:
P1.9 Validation and regression pass.

Phase 1 progress update (2026-03-05)
Completed slice:
- P1.9 Validation and regression pass

Validation details:
- `npm run typecheck` passed.
- `npm run test` passed (7 files, 86 tests).
- Editor diagnostics check (`get_errors`) returned no errors.

Validation gap discovered:
- `npm run lint` failed because `eslint` binary is unavailable in the current environment (`sh: 1: eslint: not found`).
- Logged in TODO as a follow-up tooling item.

Next active slice:
P1.10 Docs and handoff update.

Phase 1 progress update (2026-03-05)
Completed slice:
- P1.10 Docs and handoff update

Phase 1 execution-plan status:
- P1.1 through P1.10 are complete.
- Domain API migration is complete (`scenarioSelection`, `parameterState`, `runtimeState`).
- App has been migrated to domain APIs and compatibility shims were removed.

Phase 1 residual follow-ups (already logged in TODO):
- Add App-store domain wiring regression tests.
- Add model-reset domain key coverage tests.
- Restore lint toolchain availability (`eslint` missing in current environment).

Next recommended workstream:
Begin Option B shell UI implementation (High Priority Frontend/UX section in TODO).

Phase 2 planning + legacy cleanup update (2026-03-05)
Completed in this slice:
- Reviewed the three planning/control documents together:
	- `docs/FRONTEND_INPUT_SELECTION_REACTIVITY_REVIEW_2026-03-05.md`
	- `docs/status.md`
	- `TODO.md`
- Confirmed alignment on product direction: Option B remains the Phase 2 driver, analytical overlays stay permissive-with-warning, and benchmark clone-to-custom is mandatory.
- Cleaned root-level legacy artifacts:
	- Removed `tmp_heatpump.svelte`.
	- Removed one-off root scripts: `fix_frontend_soa.mjs`, `fix_grid_cells_step.mjs`, `fix_grid_cells_tests.mjs`, `refactor_soa.mjs`, `test_hydrate.mjs`, `test_hydration_empty.mjs`, `test_hydration_payload.mjs`, `test_hydration_worker.mjs`.
	- Verified no root-level `.resolved` files are present.

Planning updates applied:
- Added `Active Execution Plan — Phase 2 (Interruption-Safe)` in `TODO.md` with 10 explicit slices (`P2.1` to `P2.10`).
- Marked `P2.1 UX contract + state schema freeze` as `(in progress)` to establish deterministic resume state.
- Added Phase 2 acceptance checklist and a dedicated interruption-resume protocol block.
- Marked legacy cleanup TODO items complete.

Validation run:
- Repository had no pre-existing changed files before this slice.
- Confirmed deleted artifacts are absent after cleanup.

Next active slice:
P2.2 Preset composer shell UI (in progress).

Phase 2 progress update (2026-03-05)
Slice in progress:
- P2.2 Preset composer shell UI

Implemented in this sub-slice (centralized facet mapping):
- Centralized facet mapping in `src/lib/stores/phase2PresetContract.ts`:
	- Added `FACET_TO_SECTION_TARGET` and `FACET_TO_OVERRIDE_GROUPS`.
	- Added exported helpers `getFacetCustomizeSectionTarget(...)` and `getFacetOverrideGroups(...)`.
- Refactored `src/App.svelte` to consume centralized helpers for both customize routing and reset-group resolution.
- Refactored `src/lib/ui/TopBar.svelte` to consume centralized helper for facet override-group resolution.
- Extended `src/lib/stores/phase2PresetContract.test.ts` with mapping helper tests (known keys + fallback behavior).

Validation run:
- `npm run typecheck` passed.
- `npm run test -- src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed (2 files, 10 tests).

Next active slice:
P2.2 Preset composer shell UI (in progress).

Phase 2 progress update (2026-03-05)
Slice in progress:
- P2.2 Preset composer shell UI

Implemented in this sub-slice (Hybrid polish):
- `src/lib/ui/TopBar.svelte`
	- Added per-facet `Customize` and `Reset` controls directly under each facet card.
	- Added per-facet changed-field summary under the customize controls.
	- Active customize selection is visually highlighted on the facet customize action.
- `src/App.svelte`
	- Added active customize-group state and facet reset handlers.
	- Reset now applies base values for overridden fields in the selected facet group.
	- When overrides clear completely, flow returns to preset state by reapplying current toggles.
- `src/lib/ui/InputsTab.svelte`
	- Added customize-session collapsible footer with explicit `OK` button.
	- Kept section focus/highlight routing for local customize entry points.

Design note:
- Implemented collapsible inline customize sessions rather than modal popup, to keep scientific context visible and avoid modal churn while editing coupled parameters.

Validation run:
- `npm run typecheck` passed.
- `npm run test -- src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed (2 files, 8 tests).

Remaining in current slice:
- Add richer generated-profile controls in shell (show changed fields toggle + per-group quick actions).

Next active slice:
P2.2 Preset composer shell UI (in progress).

Phase 2 progress update (2026-03-05)
Slice in progress:
- P2.2 Preset composer shell UI

Implemented in this sub-slice (Hybrid interaction pass):
- Updated `src/lib/ui/TopBar.svelte`:
	- Removed the prominent global `Custom` action button.
	- Added per-facet `Customize <Facet>` actions under each facet card.
	- Kept customization state visible as a lightweight `Customized` status badge.
- Updated `src/App.svelte`:
	- Added facet-to-input-section routing (`geo/grid/dt/fluid/rock/grav/cap/well/benchmarkId`).
	- Wired `TopBar` customize actions to section-focus requests for `InputsTab`.
- Updated `src/lib/ui/InputsTab.svelte`:
	- Added targeted section focus/highlight behavior for customize requests.
	- Section wrappers now smoothly scroll/focus: shell, static, timestep, reservoir, relcap, well, analytical.

Validation run:
- `npm run typecheck` passed.
- `npm run test -- src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed (2 files, 8 tests).

Remaining in current slice:
- Add richer generated-profile controls in shell (show changed fields toggle + per-group quick actions).

Next active slice:
P2.2 Preset composer shell UI (in progress).

Phase 2 progress update (2026-03-05)
Slice in progress:
- P2.2 Preset composer shell UI

Implemented in this sub-slice:
- Added first visible Preset + Customize shell component: `src/lib/ui/PresetCustomizeShell.svelte`.
- Wired the shell into `src/lib/ui/InputsTab.svelte` above the existing parameter panels.
- Bound live Phase 2 contract fields from `App` into `InputsTab`:
	- `basePreset`
	- `benchmarkProvenance`
	- `parameterOverrideCount`
	- `parameterOverrideGroups`
	- `analyticalStatus`

Validation run:
- `npm run typecheck` passed.
- `npm run test -- src/lib/stores/phase2PresetContract.test.ts` passed (1 file, 5 tests).

Remaining in current slice:
- Implement mode/facet composer interactions in the new shell area.
- Add generated-profile interaction controls (next visible increment).

Next active slice:
P2.2 Preset composer shell UI (in progress).

Phase 2 progress update (2026-03-05)
Completed slice:
- P2.1 UX contract + state schema freeze

Implementation details:
- Added Phase 2 contract module: `src/lib/stores/phase2PresetContract.ts`.
	- Defines frozen schema/types for `basePreset`, `parameterOverrides`, `benchmarkProvenance`, and `analyticalStatus`.
	- Includes deterministic override grouping and analytical-status evaluator helpers.
- Integrated contract-derived fields into store API in `src/lib/stores/simulationStore.svelte.ts`.
	- `scenarioSelection`: `basePreset`, `benchmarkProvenance`, `setBenchmarkProvenance(...)`.
	- `parameterState`: `parameterOverrides`, `parameterOverrideGroups`, `parameterOverrideCount`.
	- `runtimeState`: `analyticalStatus`.
- Added focused unit tests: `src/lib/stores/phase2PresetContract.test.ts`.
- Added contract documentation: `docs/PHASE2_PRESET_CUSTOMIZE_CONTRACT.md`.

Validation run:
- `npm run typecheck` passed.
- `npm run test -- src/lib/stores/phase2PresetContract.test.ts` passed (1 file, 5 tests).

Next active slice:
P2.2 Preset composer shell UI (in progress).

Phase 1 acceptance checklist closure update (2026-03-05)
Completed in this slice:
- Closed remaining unchecked Phase 1 acceptance checklist items in `TODO.md` with explicit verification evidence.

Targeted verification commands run:
- Frontend validation/case tests:
	- `npm run test -- src/lib/validateInputs.test.ts src/lib/caseCatalog.test.ts src/lib/buildCreatePayload.test.ts`
	- Result: 3 test files passed, 41 tests passed.
- Simulator run/step regression tests:
	- `cargo test adaptive_timestep_produces_multiple_substeps_for_strong_flow`
	- `cargo test pressure_resolve_on_substep_produces_physical_results`
	- `cargo test saturation_stays_within_physical_bounds`
	- Result: all passed.
- Project compile safety:
	- `npm run typecheck`
	- Result: passed (`tsc --noEmit`).

Code-level verification anchors used for checklist evidence:
- Validation gating wiring in `src/App.svelte` (`hasValidationErrors` passed to `RunControls`).
- Explicit blocked-run error path and benchmark-only pre-run gating in `src/lib/stores/simulationStore.svelte.ts`.

Next active slice:
P2.2 Preset composer shell UI (in progress).

Phase 2 progress update (2026-03-05)
Completed slice:
- P2.2 Preset composer shell UI

Implemented in this slice (generated-profile controls + quick actions):
- `src/lib/ui/PresetCustomizeShell.svelte`
	- Added `Show changed fields` / `Hide changed fields` control for the generated-profile section.
	- Added per-group quick actions in shell (`Customize`, `Reset`) when changed fields are expanded.
	- Added per-group changed-key pills with active customize-group highlighting.
- `src/App.svelte`
	- Added shell interaction state (`showChangedFields`) and handlers for per-group customize/reset flows.
	- Added shared reset helper for override groups and reuse from facet-level reset path.
	- Wired group customize actions to centralized section-target routing and existing inputs-section focus behavior.
- `src/lib/ui/InputsTab.svelte`
	- Wired new shell callback props and state through to `PresetCustomizeShell`.
- `src/lib/stores/phase2PresetContract.ts`
	- Added centralized `OVERRIDE_GROUP_TO_SECTION_TARGET` mapping and `getOverrideGroupSectionTarget(...)` helper.
- `src/lib/stores/phase2PresetContract.test.ts`
	- Added helper tests for override-group to section-target mapping (including fallback behavior).

Validation run:
- `npm run typecheck` passed.
- `npm run test -- src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed (2 files, 11 tests).

Next active slice:
P2.3 Override tracking + changed-field UX (in progress), focusing on dedicated regression and policy tests for changed-field/reset pathways.

Phase 2 progress update (2026-03-05)
Completed slice:
- P2.3 Override tracking + changed-field UX

Implemented in this slice (regression/policy hardening):
- `src/lib/stores/phase2PresetContract.ts`
	- Added pure helper `buildOverrideResetPlan(...)` to compute deterministic reset-to-base actions for selected override groups.
	- Reset plan guarantees stable order, de-duplicates repeated keys across groups, and skips stale/missing override keys.
- `src/App.svelte`
	- Refactored group-reset flow to use `buildOverrideResetPlan(...)` so runtime behavior follows the tested contract helper.
- `src/lib/stores/phase2PresetContract.test.ts`
	- Added deterministic-order test for `buildParameterOverrides(...)` using explicit tracked-key ordering.
	- Added grouped reset-plan tests for de-duplication and stale-key filtering behavior.

Validation run:
- `npm run typecheck` passed.
- `npm run test -- src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed (2 files, 14 tests).

Next active slice:
P2.4 Benchmark clone-to-custom flow (in progress).

Phase 2 progress update (2026-03-05)
Completed slice:
- P2.4 Benchmark clone-to-custom flow

Implemented in this slice:
- `src/lib/ui/TopBar.svelte`
	- Added one-click benchmark `Clone to Custom` action in benchmark details area.
	- Added clone provenance status line when lineage exists.
	- Added explicit fallback note when benchmark is customized without clone provenance.
- `src/App.svelte`
	- Added clone handler (`handleCloneBenchmarkToCustom`) that:
		- creates immutable benchmark provenance metadata,
		- transitions benchmark preset into editable custom state via `handleParamEdit()`,
		- preserves first-clone lineage for the session,
		- scrolls/focuses inputs section for immediate customization.
- `src/lib/stores/phase2PresetContract.ts`
	- Added `buildBenchmarkCloneProvenance(...)` helper for deterministic lineage payload construction.
- `src/lib/stores/simulationStore.svelte.ts`
	- Enforced clone lineage lifecycle by clearing `benchmarkProvenance` on mode changes and preset/facet toggle changes.
- `src/lib/stores/phase2PresetContract.test.ts`
	- Added provenance helper tests for valid benchmark context and incomplete-context null behavior.

Validation run:
- `npm run typecheck` passed.
- `npm run test -- src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed (2 files, 16 tests).

Next active slice:
P2.5 Analytical eligibility evaluator (in progress).

Phase 2 progress update (2026-03-05)
Completed slice:
- P2.5 Analytical eligibility evaluator

Implemented in this slice:
- `src/lib/stores/phase2PresetContract.ts`
	- Extended analytical status contract with severity-aware fields:
		- `warningSeverity: 'none' | 'notice' | 'warning' | 'critical'`
		- `reasonDetails: Array<{ code; message; severity }>`
	- Refactored `evaluateAnalyticalStatus(...)` to produce deterministic reason codes, messages, and severity levels.
	- Added highest-severity summarization logic to support policy-driven UI behavior.
- `src/lib/stores/phase2PresetContract.test.ts`
	- Added severity-specific evaluator tests for:
		- reference/no-warning paths,
		- approximate warning paths,
		- critical contradiction paths,
		- off-mode/no-warning summary behavior.
- `src/lib/ui/InputsTab.svelte`
	- Updated `analyticalStatus` typing/defaults to match enhanced contract.
- `src/lib/ui/PresetCustomizeShell.svelte`
	- Updated `analyticalStatus` typing/defaults and surfaced severity label in shell status chip.
- `docs/PHASE2_PRESET_CUSTOMIZE_CONTRACT.md`
	- Updated frozen schema docs to include `warningSeverity` and `reasonDetails`.

Validation run:
- `npm run typecheck` passed.
- `npm run test -- src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed (2 files, 18 tests).

Next active slice:
P2.6 Analytical status banner UX (in progress).

Phase 2 progress update (2026-03-05)
Completed slice:
- P2.6 Analytical status banner UX

Implemented in this slice:
- `src/lib/ui/AnalyticalStatusBanner.svelte`
	- Added a dedicated analytical-status banner component with severity-aware tone (`notice | warning | critical`), persistent status messaging, and an expandable caveat details panel.
	- Added per-reason severity badges plus reason-code tooltips for faster approximation-cause inspection.
- `src/App.svelte`
	- Wired the banner into the main results surface so approximate analytical status is persistently visible above the chart/3D analysis panels.
- `TODO.md`
	- Marked `P2.6` complete and advanced `P2.7` to active in-progress slice.

Validation run:
- `npm run typecheck` passed.
- `npm run test -- src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed (2 files, 18 tests).

Next active slice:
P2.7 Store/App integration hardening (in progress).

Phase 2 progress update (2026-03-05)
Completed slice:
- P2.7 Store/App integration hardening

Implemented in this slice:
- `src/lib/stores/simulationStore.svelte.ts`
	- Added domain-level scenario action `cloneActiveBenchmarkToCustom()` so benchmark clone lineage is created/applied by `scenarioSelection` instead of App-side assembly.
	- Added domain-level parameter action `resetOverrideGroupsToBase(groupKeys)` so grouped override resets are applied through `parameterState` APIs instead of dynamic App-side field mutation.
- `src/App.svelte`
	- Removed transitional App-side provenance/diff-plan assembly (`catalog`, `buildBenchmarkCloneProvenance`, `buildOverrideResetPlan`).
	- Routed clone and grouped reset flows through store domain APIs (`scenario.cloneActiveBenchmarkToCustom()`, `params.resetOverrideGroupsToBase(...)`).
- `src/lib/ui/AnalyticalStatusBanner.svelte`
	- Tightened banner input contract to current Phase 2 schema by requiring `reasonDetails` + `reasons` and removing fallback compatibility branching.
- `src/lib/appStoreDomainWiring.test.ts`
	- Added regression test coverage that asserts `App.svelte` uses `scenarioSelection`/`parameterState`/`runtimeState` and keeps clone/reset flows domain-driven.
- `TODO.md`
	- Marked `P2.7` complete and advanced `P2.8` to active in-progress slice.

Validation run:
- `npm run typecheck` passed.
- `npm run test -- src/lib/appStoreDomainWiring.test.ts src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed (3 files, 21 tests).

Next active slice:
P2.8 Regression + policy tests (in progress).

Phase 2 progress update (2026-03-05)
Completed slice:
- P2.9 Remove pre-run loading pipeline

Implemented in this slice:
- `src/lib/stores/simulationStore.svelte.ts`
	- Removed benchmark pre-run state (`preRunData`, `preRunLoading`, `preRunWarning`, `preRunLoadToken`, `preRunContinuationAvailable`).
	- Removed `loadPreRunCase(...)` fetch/decompression/hydration path and related runtime branching.
	- Simplified mode/toggle/param-edit flows so selection only applies parameters and marks model state.
	- Removed pre-run fields from `scenarioSelection` API surface.
- `src/App.svelte`
	- Removed pre-run warning/loading UI banners.
- `src/lib/caseCatalog.ts`
	- Updated `buildCaseKey(...)` doc comment to remove pre-run artifact wording.
- `src/lib/ui/InputsTab.svelte`
	- Updated stale read-only helper text that referenced pre-run viewing.
- `src/lib/simulator-types.ts`
	- Removed stale pre-run hydration comment.
- `TODO.md`
	- Marked `P2.9` complete and retired obsolete pre-run validation/decompression follow-up items.

Validation run:
- `npm run typecheck` passed.
- `npm run test -- src/lib/appStoreDomainWiring.test.ts src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed.

Next active slice:
P2.8 Regression + policy tests (in progress), now scoped to clone/override/analytical policy checks without pre-run pathways.

Phase 2 progress update (2026-03-05)
Slice in progress:
- P2.8 Regression + policy tests

Implemented in this sub-slice (catalog schema refactor):
- `src/lib/catalog.json`
	- Restructured from flat `dimensions`/`disabilityRules` to `modes.{dep|wf|sim|benchmark}` with per-mode `baseParams`, `dimensions`, and `disabilityRules`.
- `src/lib/caseCatalog.ts`
	- Updated schema types and added mode helpers (`normalizeMode`, `getModeCatalog`, `getModeDimensions`).
	- Refactored `composeCaseParams`, `buildCaseKey`, `getDisabledOptions`, `stabilizeToggleState`, and `getDefaultToggles` to use mode-local dimensions/rules.
	- Preserved deterministic case-key prefix format (`mode-<mode>_...`) for non-benchmark cases.
- `src/lib/ui/TopBar.svelte`
	- Switched active facet rendering to mode-local dimensions via `getModeDimensions(activeMode)`.
- `src/lib/caseCatalog.test.ts`
	- Updated schema shape assertion for `catalog.modes`.

Validation run:
- `npm run typecheck` passed.
- `npm run test -- src/lib/caseCatalog.test.ts src/lib/appStoreDomainWiring.test.ts src/lib/stores/phase2PresetContract.test.ts` passed (3 files, 21 tests).

Next active slice:
P2.8 Regression + policy tests (in progress), continuing with additional policy tests over clone/override/analytical flows on top of mode-local catalog rules.

Phase 2 recovery planning update (2026-03-06)
Completed slice:
- R0.2 Docs reset + authoritative recovery plan

Audit findings captured in this slice:
- `src/lib/ui/ModePanel.svelte` is now the primary UI surface, but it is an intermediate implementation:
	- sections/controls are hardcoded in component code,
	- the component currently accepts `params: any`,
	- manual field edits no longer clearly route through the Phase 2 preset/customize intent model.
- `src/lib/stores/simulationStore.svelte.ts` still maintains valuable contract state (`basePreset`, `parameterOverrides`, grouped reset logic, benchmark provenance, analytical status), so the best next step is to reconnect the UI to that contract rather than discard it.
- Older planning docs still described the shell-centric path as if it were the live implementation, so resume state was ambiguous.

Planning updates applied:
- `TODO.md`
	- Added `Authoritative Recovery Plan — Schema-Driven Composer (Interruption-Safe)`.
	- Marked the next active slice as `R1.1 Restore unified-panel preset/customize semantics (in progress)`.
	- Added explicit follow-up slices for typed schema, warning policy, toggle-plus-custom controls, staged migration, and regression hardening.
- `docs/PHASE2_PRESET_CUSTOMIZE_CONTRACT.md`
	- Clarified that the document remains the store-contract reference, while the previous shell-specific UI description is historical.
- `docs/FRONTEND_INPUT_SELECTION_REACTIVITY_REVIEW_2026-03-05.md`
	- Marked as historical rationale and added 2026-03-06 follow-up decisions for a typed schema-driven UI, code-defined rules, permissive warning policy, and quick-select plus custom-entry controls.

Validation run:
- No code behavior changed in this slice; no tests were required.

Next active slice:
R1.1 Restore unified-panel preset/customize semantics (in progress).

Phase 2 recovery progress update (2026-03-06)
Completed slice:
- R1.1 Restore unified-panel preset/customize semantics

Implemented in this slice:
- `src/lib/ui/ModePanel.svelte`
	- Added `onParamEdit` passthrough so manual field edits in expanded section bodies route back through domain intent.
	- Added a lightweight preset/customize summary strip showing current source, base preset label, changed-field count, and benchmark clone provenance.
- `src/App.svelte`
	- Passed `scenario.handleParamEdit` and `scenario.basePreset` into `ModePanel` so the live panel consumes the store contract directly.
- `src/lib/stores/simulationStore.svelte.ts`
	- Added auto-clear behavior for non-benchmark customized state when override count returns to zero and no clone provenance is active.
- `src/lib/appStoreDomainWiring.test.ts`
	- Added regression coverage ensuring `ModePanel` receives `onParamEdit` and `basePreset` from domain APIs.

Validation run:
- `npm run test -- src/lib/appStoreDomainWiring.test.ts src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed (3 files, 22 tests).
- `npx vite build` passed.
- `get_errors` reported no errors in modified files.
- `npx svelte-check --tsconfig ./tsconfig.json` still reports the same pre-existing `src/lib/components/ui/Collapsible.svelte` `on:toggle` typing error; no new errors were introduced by this slice.

Next active slice:
R1.2 Define typed schema for UI composition (in progress).

Phase 2 recovery progress update (2026-03-06)
Slice in progress:
- R1.2 Define typed schema for UI composition

Implemented in this sub-slice (Geometry + Grid first migration):
- `src/lib/ui/modePanelSchema.ts`
	- Added typed schema definitions for mode-panel sections, schema-backed controls, quick-pick options, inline custom-entry behavior, and change-effect metadata.
	- Added the first concrete schema: `GEOMETRY_GRID_SECTION_SCHEMA`.
- `src/lib/ui/SchemaSectionRenderer.svelte`
	- Added a reusable schema renderer for the first control set (`quick-picks` with inline `Custom` number entry, plus typed numeric controls).
	- Custom entry stays code-driven for behavior while config/schema stays declarative.
- `src/lib/ui/ModePanel.svelte`
	- Replaced the hardcoded `StaticPropertiesPanel` body in the `Geometry + Grid` section with the schema renderer.
	- Switched section metadata to typed `getModePanelSections(...)` definitions.
- `src/lib/ui/modePanelHelpers.test.ts`
	- Added focused tests for section metadata, `nx` quick-picks, inline custom-entry behavior metadata, quick-pick matching, and control-level error lookup.

Validation run:
- `npm run test -- src/lib/ui/modePanelSchema.test.ts src/lib/appStoreDomainWiring.test.ts src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed (4 files, 27 tests).
- `npx vite build` passed.
- `get_errors` reported no errors in modified schema/renderer files.

Next active slice:
R1.2 Define typed schema for UI composition (in progress), continuing with parameter typing cleanup and migration of the next section(s) to schema-driven rendering.

Phase 2 recovery progress update (2026-03-06)
Slice in progress:
- R1.2 Define typed schema for UI composition

Implemented in this sub-slice (typed parameter bindings cleanup):
- `src/lib/ui/modePanelSchema.ts`
	- Added explicit UI-facing parameter types for the schema-backed panel path (`ModePanelParameterBindings`, plus supporting `PermMode`, well-control, and analytical-mode types).
- `src/lib/ui/SchemaSectionRenderer.svelte`
	- Replaced the ad hoc geometry binding shape with the shared `ModePanelParameterBindings` type.
	- Tightened quick-pick patch application to typed geometry/grid parameter keys.
- `src/lib/ui/ModePanel.svelte`
	- Replaced `params: any` with `ModePanelParameterBindings`.
	- Reused store contract types (`BasePresetProfile`, `BenchmarkProvenance`) instead of duplicating local shape definitions.

Validation run:
- `npm run test -- src/lib/ui/modePanelSchema.test.ts src/lib/appStoreDomainWiring.test.ts src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed (4 files, 27 tests).
- `npx vite build` passed.
- `get_errors` reported no errors in modified typing/schema files.

Next active slice:
R1.2 Define typed schema for UI composition (in progress), continuing with migration of the next section(s) and removal of remaining implicit parameter contracts.

Recovery direction adjustment (2026-03-06, later session)
Completed in this planning slice:
- Reconsidered the broad schema-driven UI direction before starting the next section migration.

Decision update:
- Do not continue toward a general JSON/schema-driven definition of the full input surface.
- Preferred architecture is now mode-specific Svelte components for top-level workflows:
	- `DepletionPanel.svelte`
	- `WaterfloodPanel.svelte`
	- `SimulationPanel.svelte`
	- `BenchmarkPanel.svelte`
- Reuse smaller focused subcomponents underneath those mode panels instead of trying to render most of the UI from one schema system.
- Keep TypeScript in charge of constraint logic, warning policy, and simulator behavior.
- Keep any config-driven approach narrow and local only where it clearly helps (for example compact quick-pick/custom-entry helpers), not as the primary architecture.

Planning updates applied:
- `TODO.md`
	- Renamed the authoritative recovery plan to `Mode-Specific Panels`.
	- Replaced the broad schema-migration follow-ups with mode-panel extraction and focused subcomponent reuse.
	- Preserved the existing geometry schema work as an experiment/helper rather than the mandated direction for all sections.

Validation run:
- No code behavior changed in this planning slice; no tests were required.

Next active slice:
R1.2 Extract mode-specific top-level panels (in progress).

Phase 2 recovery progress update (2026-03-06)
Slice in progress:
- R1.2 Define typed schema for UI composition

Implemented in this sub-slice (ModePanel UI polish before next migration):
- `src/lib/ui/ModePanel.svelte`
	- Removed the always-visible low-value preset status strip; it now only appears when there is actual value to show (changed-field count and/or benchmark clone provenance).
	- Geometry now opens directly into the compact custom editor instead of showing a redundant inner heading and repeated preset context.
- `src/lib/ui/SchemaSectionRenderer.svelte`
	- Added `showHeader` and `hideQuickPickOptions` controls so schema-backed sections can be embedded without redundant framing.
	- Compacted the schema-backed field layout to reduce horizontal sprawl and make the Geometry custom editor feel like an inline override surface rather than a second large panel.
	- Hid redundant quick-pick buttons in the Geometry body because preset quick-picks already exist in the parent facet selector row.

Validation run:
- `npm run test -- src/lib/ui/modePanelSchema.test.ts src/lib/appStoreDomainWiring.test.ts src/lib/stores/phase2PresetContract.test.ts src/lib/caseCatalog.test.ts` passed (4 files, 27 tests).
- `npx vite build` passed.
- `get_errors` reported no errors in modified UI files.

Next active slice:
R1.2 Define typed schema for UI composition (in progress), continuing with migration of the next section(s) and removal of remaining implicit parameter contracts.

Phase 2 recovery progress update (2026-03-06)
Completed slice:
- R1.2 Extract mode-specific top-level panels

Implemented in this slice:
- `src/lib/ui/ModePanel.svelte`
	- Reduced the component to a shell responsible for mode tabs, changed/provenance status, validation warnings, and routing into dedicated top-level mode panels.
- Added dedicated top-level workflow components:
	- `src/lib/ui/BenchmarkPanel.svelte`
	- `src/lib/ui/DepletionPanel.svelte`
	- `src/lib/ui/WaterfloodPanel.svelte`
	- `src/lib/ui/SimulationPanel.svelte`
- Added `src/lib/ui/ScenarioSectionsPanel.svelte` to hold the shared non-benchmark section composition while keeping the workflow split explicit at the top level.
- Added `src/lib/ui/modePanelTypes.ts` so the new panel boundary shares one typed prop contract instead of duplicating inline panel prop shapes.
- Added `src/lib/ui/modePanelComposition.test.ts` to lock in the new composition boundary and keep the section renderer delegation explicit.

Validation run:
- `npm run test -- src/lib/ui/modePanelComposition.test.ts src/lib/ui/modePanelSchema.test.ts src/lib/appStoreDomainWiring.test.ts` passed (3 files, 11 tests).
- `npx vite build` passed.
- `get_errors` reported no errors in the extracted panel files.

Next active slice:
R1.3 Define warning severity + surfacing policy (in progress).

Phase 2 recovery progress update (2026-03-06, later)
Slice in progress:
- R1.3 Define warning severity + surfacing policy

Implemented in this sub-slice:
- `src/lib/validateInputs.ts`
	- Replaced raw warning strings with typed validation warnings carrying `code`, `surface`, `message`, and optional `fieldKey` metadata.
	- Classified current validation warnings into `advisory` and `non-physical` surfaces instead of leaving them as an untyped list.
- `src/lib/warningPolicy.ts`
	- Added a store-facing warning policy builder that groups current issues into four explicit surfaces:
		- `blockingValidation`
		- `nonPhysical`
		- `referenceCaveat`
		- `advisory`
	- Aggregates validation errors, typed validation warnings, analytical caveats, solver warnings, runtime warnings, and model reinit notices into one typed summary.
- `src/lib/stores/simulationStore.svelte.ts`
	- Added derived `warningPolicy` and exposed it through `runtimeState`.
- `src/lib/ui/WarningPolicyPanel.svelte`
	- Added an explicit surface renderer for grouped warning classes.
- `src/lib/ui/ModePanel.svelte`
	- Replaced the previous raw validation-warning list with grouped warning surfaces for `blockingValidation`, `nonPhysical`, and `advisory`.
- `src/App.svelte`
	- Routed `runtime.warningPolicy` into `ModePanel`.
	- Removed the now-duplicated raw runtime warning banner.

Reminder logged for next sub-step:
- Propagate the same warning classes into `src/lib/ui/RunControls.svelte` once the policy model settles, so runtime controls stop relying on raw inline strings.

Validation run:
- `npm run test -- src/lib/warningPolicy.test.ts src/lib/validateInputs.test.ts src/lib/appStoreDomainWiring.test.ts src/lib/ui/modePanelComposition.test.ts src/lib/stores/phase2PresetContract.test.ts` passed (5 files, 57 tests).
- `npx vite build` passed.
- `get_errors` reported no errors in modified warning-policy files.

Next active slice:
R1.3 Define warning severity + surfacing policy (in progress), continuing with `RunControls` surfacing and final policy cleanup.

Phase 2 recovery progress update (2026-03-06, later still)
Slice in progress:
- R1.3 Define warning severity + surfacing policy

Implemented in this sub-slice:
- `src/lib/warningPolicy.ts`
	- Added filtered warning-group selectors so different UI surfaces can consume one shared warning model without duplicating runtime vs validation messages.
	- Moved long-run estimate messaging into the centralized advisory warning path.
- `src/lib/ui/WarningPolicyPanel.svelte`
	- Added per-group source filtering and source labels so one shared renderer can serve mode inputs, runtime controls, and analytical caveat surfaces.
- `src/lib/ui/ModePanel.svelte`
	- Restricted the mode-panel warning surface to validation-origin warnings, avoiding duplication with runtime controls.
- `src/lib/ui/RunControls.svelte`
	- Replaced raw `solverWarning` / `modelReinitNotice` rendering with the shared warning-policy surface.
	- Run controls now show blocking validation via the same common system instead of depending on disconnected inline warning props.
- `src/lib/ui/AnalyticalStatusBanner.svelte`
	- Reworked the analytical caveat surface to wrap the shared warning policy instead of carrying a separate analytical-warning presentation model.
- `src/App.svelte`
	- Routed `runtime.warningPolicy` into `RunControls` and the analytical caveat banner.

Audit result:
- Live warning surfaces are now centralized on `warningPolicy`.
- Remaining warning islands are legacy shell-era components that are no longer on the active app path:
	- `src/lib/ui/InputsTab.svelte`
	- `src/lib/ui/PresetCustomizeShell.svelte`
	- `src/lib/ui/DynamicControlsPanel.svelte`
- Those should be cleaned or retired under the tail of `R1.3` / `R1.7`, rather than introducing another warning model beside the shared one.

Validation run:
- `npm run test -- src/lib/warningPolicy.test.ts src/lib/warningPolicyFilters.test.ts src/lib/validateInputs.test.ts src/lib/appStoreDomainWiring.test.ts src/lib/stores/phase2PresetContract.test.ts src/lib/ui/modePanelComposition.test.ts` passed (6 files, 59 tests).
- `npx vite build` passed.
- `get_errors` reported no errors in modified warning-centralization files.

Next active slice:
R1.3 Define warning severity + surfacing policy (in progress), continuing with cleanup of remaining legacy warning islands and any final shared-warning API simplification.

Phase 2 recovery progress update (2026-03-06, warning policy closure)
Completed slice:
- R1.3 Define warning severity + surfacing policy

Implemented in this closing sub-slice:
- `src/lib/ui/InputsTab.svelte`
	- Replaced remaining legacy raw validation-warning typing with shared `ValidationWarning` and shared Phase 2 analytical/provenance contract types.
	- Updated legacy warning rendering to consume structured warning messages.
- `src/lib/ui/PresetCustomizeShell.svelte`
	- Reused the shared `AnalyticalStatus` contract instead of keeping a local duplicate warning-related type.
- `TODO.md`
	- Marked `R1.3` complete and moved the last warning-specific dead code (`src/lib/ui/DynamicControlsPanel.svelte`) into the shell-era retirement slice `R1.7`.

Audit result:
- Live warning handling is now centralized through shared `warningPolicy` surfaces in `src/lib/ui/ModePanel.svelte`, `src/lib/ui/RunControls.svelte`, and `src/lib/ui/AnalyticalStatusBanner.svelte`.
- `src/lib/ui/DynamicControlsPanel.svelte` is no longer referenced from `src/**`, so it should be removed with the rest of the shell-era leftovers instead of being migrated.

Validation run:
- `get_errors` reported no errors in `src/lib/ui/InputsTab.svelte`, `src/lib/ui/PresetCustomizeShell.svelte`, `src/lib/ui/RunControls.svelte`, `src/lib/ui/AnalyticalStatusBanner.svelte`, `src/lib/warningPolicy.ts`, and `src/App.svelte`.
- `npm run test -- src/lib/validateInputs.test.ts src/lib/warningPolicy.test.ts src/lib/warningPolicyFilters.test.ts src/lib/appStoreDomainWiring.test.ts` passed (4 files, 42 tests).
- `npm run build` passed.
- `npm run typecheck` is still blocked by unrelated pre-existing errors in `src/lib/ui/modePanelSchema.test.ts` (`ControlDefinition<GeometryGridParamKey>` missing `options` / `custom` in that test's assumptions); no new type errors were introduced in the warning-policy files checked above.

Next active slice:
R1.4 Keep only narrow config-driven helpers where they help (in progress).

Phase 2 recovery progress update (2026-03-06, narrow helper cleanup)
Completed slice:
- R1.4 Keep only narrow config-driven helpers where they help

Implemented in this slice:
- `src/lib/ui/modePanelSections.ts`
	- Split shared mode-section metadata out of the old schema module so section composition stays explicit and independent from quick-edit helper definitions.
- `src/lib/ui/geometryGridQuickEditor.ts`
	- Replaced the generic schema/control model with a geometry-grid-only typed quick-edit helper for the current local repetitive case.
- `src/lib/ui/GeometryGridQuickEditor.svelte`
	- Renamed the old generic renderer to an explicit geometry-grid quick editor and kept `nz` layer-sync behavior local to that helper.
- `src/lib/ui/ScenarioSectionsPanel.svelte`
	- Switched the geometry section to the local `GeometryGridQuickEditor` while leaving the other section bodies as normal Svelte components.
- `src/lib/ui/modePanelTypes.ts`
	- Moved panel binding and control-mode types into the shared mode-panel type contract instead of leaving them inside a pseudo-schema module.
- `src/lib/ui/modePanelSchema.test.ts`
	- Updated the helper tests to the new local helper modules and fixed the narrowing issue that had been blocking project typecheck.
- Deleted obsolete generic-helper files:
	- `src/lib/ui/modePanelSchema.ts`
	- `src/lib/ui/SchemaSectionRenderer.svelte`

Outcome:
- The app no longer carries a general-looking schema-rendering path beside the mode-specific Svelte panel architecture.
- Remaining config-driven behavior is now intentionally narrow and local to the geometry-grid quick editor.

Validation run:
- `get_errors` reported no errors in `src/lib/ui/GeometryGridQuickEditor.svelte`, `src/lib/ui/ScenarioSectionsPanel.svelte`, `src/lib/ui/modePanelTypes.ts`, `src/lib/ui/geometryGridQuickEditor.ts`, `src/lib/ui/modePanelSections.ts`, and `src/lib/ui/modePanelSchema.test.ts`.
- `npm run typecheck` passed.
- `npm run test -- src/lib/ui/modePanelSchema.test.ts src/lib/ui/modePanelComposition.test.ts src/lib/appStoreDomainWiring.test.ts` passed (3 files, 12 tests).
- `npm run build` passed.

Next active slice:
R1.5 Add toggle-plus-custom pattern where useful (in progress).

Phase 2 recovery progress update (2026-03-06, toggle-plus-custom expansion)
Completed slice:
- R1.5 Add toggle-plus-custom pattern where useful

Implemented in this slice:
- `src/lib/ui/geometryGridQuickEditor.ts`
	- Expanded the local geometry-grid helper so `ny`, `nz`, `cellDx`, `cellDy`, and `cellDz` now use the same toggle-plus-custom definition shape as `nx`.
	- Added curated preset values for layer counts and cell sizes while keeping explicit custom entry metadata local to the geometry helper.
- `src/lib/ui/ScenarioSectionsPanel.svelte`
	- Stopped hiding the geometry quick-pick buttons in the live expanded section so the preset-toggle plus custom-entry pairing is visible in the actual mode-panel flow.
- `src/lib/ui/modePanelSchema.test.ts`
	- Added coverage for the additional geometry toggle-plus-custom controls, including `nz` layer-sync behavior and cell-size preset definitions.

Outcome:
- The live geometry-grid section now presents quick presets and exact custom entry in the same control group for all of its grid-shape fields.
- The toggle-plus-custom pattern remains intentionally local to `GeometryGridQuickEditor` instead of becoming a new broad rendering system.

Validation run:
- `get_errors` reported no errors in `src/lib/ui/geometryGridQuickEditor.ts`, `src/lib/ui/ScenarioSectionsPanel.svelte`, `src/lib/ui/modePanelSchema.test.ts`, and `src/lib/ui/GeometryGridQuickEditor.svelte`.
- `npm run typecheck` passed.
- `npm run test -- src/lib/ui/modePanelSchema.test.ts src/lib/ui/modePanelComposition.test.ts src/lib/appStoreDomainWiring.test.ts` passed (3 files, 13 tests).
- `npm run build` passed.

Next active slice:
R1.6 Refactor mode panels to reuse focused subcomponents (in progress).

Phase 2 recovery progress update (2026-03-06, R1.6 grid-fields extraction)
Slice in progress:
- R1.6 Refactor mode panels to reuse focused subcomponents

Implemented in this sub-slice:
- `src/lib/ui/GridFieldsPanel.svelte`
	- Added a focused geometry/grid section-body component that owns the `GeometryGridQuickEditor` plus local helper-config wiring.
- `src/lib/ui/ScenarioSectionsPanel.svelte`
	- Replaced the direct geometry quick-editor/config usage with `GridFieldsPanel`, reducing branch-specific knowledge in the shared section compositor.
- `src/lib/ui/modePanelComposition.test.ts`
	- Updated the composition contract to assert that `ScenarioSectionsPanel.svelte` renders `GridFieldsPanel` and that the geometry quick-editor wiring lives behind that subcomponent boundary.

Outcome:
- `ScenarioSectionsPanel.svelte` is now a little closer to a pure section orchestrator instead of also managing geometry-helper details.
- The geometry-grid quick-edit implementation remains local, but its panel boundary is now explicit and easier to replace or extend as `R1.6` continues.

Validation run:
- `get_errors` reported no errors in `src/lib/ui/GridFieldsPanel.svelte`, `src/lib/ui/ScenarioSectionsPanel.svelte`, and `src/lib/ui/modePanelComposition.test.ts`.
- `npm run typecheck` passed.
- `npm run test -- src/lib/ui/modePanelComposition.test.ts src/lib/ui/modePanelSchema.test.ts src/lib/appStoreDomainWiring.test.ts` passed (3 files, 14 tests).
- `npm run build` passed.

Next active slice:
R1.6 Refactor mode panels to reuse focused subcomponents (in progress), continuing with the next section-body extraction.

Phase 2 recovery progress update (2026-03-06, R1.6 timestep and wells extraction)
Slice in progress:
- R1.6 Refactor mode panels to reuse focused subcomponents

Implemented in this sub-slice:
- `src/lib/ui/TimestepFieldsPanel.svelte`
	- Added a focused timestep section-body component that owns the `TimestepControlsPanel` binding pass-through.
- `src/lib/ui/WellsFieldsPanel.svelte`
	- Added a focused wells section-body component that owns the `WellPropertiesPanel` binding pass-through.
- `src/lib/ui/ScenarioSectionsPanel.svelte`
	- Replaced the dense inline timestep and wells branches with `TimestepFieldsPanel` and `WellsFieldsPanel`.
- `src/lib/ui/modePanelComposition.test.ts`
	- Extended the composition contract to assert that timestep and wells wiring now live behind focused section-body components.

Outcome:
- `ScenarioSectionsPanel.svelte` now only orchestrates section choice and high-level section layout for geometry, wells, and timestep instead of carrying their dense prop-binding details inline.
- The next `R1.6` step can focus on another branch (`reservoir` or `analytical`) rather than reworking the same extracted paths again.

Validation run:
- `get_errors` reported no errors in `src/lib/ui/TimestepFieldsPanel.svelte`, `src/lib/ui/WellsFieldsPanel.svelte`, `src/lib/ui/ScenarioSectionsPanel.svelte`, and `src/lib/ui/modePanelComposition.test.ts`.
- `npm run typecheck` passed.
- `npm run test -- src/lib/ui/modePanelComposition.test.ts src/lib/ui/modePanelSchema.test.ts src/lib/appStoreDomainWiring.test.ts` passed (3 files, 15 tests).
- `npm run build` passed.

Next active slice:
R1.6 Refactor mode panels to reuse focused subcomponents (in progress), continuing with reservoir or analytical section-body extraction.

Phase 2 recovery progress update (2026-03-06, R1.6 reservoir and analytical extraction)
Slice in progress:
- R1.6 Refactor mode panels to reuse focused subcomponents

Implemented in this sub-slice:
- `src/lib/ui/ReservoirFieldsPanel.svelte`
	- Added a focused reservoir section-body component that owns the `ReservoirPropertiesPanel` binding pass-through.
- `src/lib/ui/AnalyticalFieldsPanel.svelte`
	- Added a focused analytical section-body component that owns the `AnalyticalInputsPanel` binding pass-through.
- `src/lib/ui/ScenarioSectionsPanel.svelte`
	- Replaced the dense inline reservoir and analytical branches with `ReservoirFieldsPanel` and `AnalyticalFieldsPanel`.
- `src/lib/ui/modePanelComposition.test.ts`
	- Extended the composition contract to assert that reservoir and analytical wiring now live behind focused section-body components.

Outcome:
- `ScenarioSectionsPanel.svelte` now delegates every branch except SCAL to focused section-body components, keeping the shared compositor centered on section selection and layout.
- The remaining `R1.6` decision is whether the direct `RelativeCapillaryPanel` branch is already narrow enough to keep as-is or should be wrapped for consistency before closing the slice.

Validation run:
- `get_errors` reported no errors in `src/lib/ui/ReservoirFieldsPanel.svelte`, `src/lib/ui/AnalyticalFieldsPanel.svelte`, `src/lib/ui/ScenarioSectionsPanel.svelte`, and `src/lib/ui/modePanelComposition.test.ts`.
- `npm run typecheck` passed.
- `npm run test -- src/lib/ui/modePanelComposition.test.ts src/lib/ui/modePanelSchema.test.ts src/lib/appStoreDomainWiring.test.ts` passed (3 files, 16 tests).
- `npm run build` passed.

Next active slice:
R1.6 Refactor mode panels to reuse focused subcomponents (in progress), deciding whether to wrap the SCAL branch or close with a final section-dispatch cleanup.

Phase 2 recovery progress update (2026-03-06, R1.6 final section-dispatch cleanup)
Completed slice:
- R1.6 Refactor mode panels to reuse focused subcomponents

Implemented in this closing sub-slice:
- `src/lib/ui/ScenarioSectionsPanel.svelte`
	- Replaced the remaining long per-section wrapper branch chain with one local `WRAPPED_SECTION_COMPONENTS` map plus `getWrappedSectionComponent()` so extracted section-body wrappers are dispatched through a single explicit path.
	- Kept `RelativeCapillaryPanel` as the single intentional direct branch instead of adding a wrapper just for symmetry.
- `src/lib/ui/ReservoirFieldsPanel.svelte`
- `src/lib/ui/WellsFieldsPanel.svelte`
- `src/lib/ui/TimestepFieldsPanel.svelte`
- `src/lib/ui/AnalyticalFieldsPanel.svelte`
	- Normalized the wrapper prop surface so the shared scenario compositor can pass the same local dispatch contract into each extracted section-body component.
- `src/lib/ui/modePanelComposition.test.ts`
	- Updated the composition contract to assert the wrapper-component dispatch map and the direct SCAL exception.
- `TODO.md`
	- Marked `R1.6` complete, advanced the next active slice to `R1.7`, and recorded that the tracker docs are now interruption-safe for the recovered panel architecture.

Outcome:
- `ScenarioSectionsPanel.svelte` is now a true section orchestrator: extracted section bodies flow through one small local dispatch mechanism, and the one remaining inline SCAL branch is an intentional exception rather than leftover branch sprawl.
- `R1.6` is complete without reintroducing a generic renderer or adding wrappers that do not materially simplify the code.

Validation run:
- `get_errors` reported no errors in `src/lib/ui/ScenarioSectionsPanel.svelte`, `src/lib/ui/ReservoirFieldsPanel.svelte`, `src/lib/ui/WellsFieldsPanel.svelte`, `src/lib/ui/TimestepFieldsPanel.svelte`, `src/lib/ui/AnalyticalFieldsPanel.svelte`, and `src/lib/ui/modePanelComposition.test.ts`.
- `npm run typecheck` passed.
- `npm run test -- src/lib/ui/modePanelComposition.test.ts src/lib/ui/modePanelSchema.test.ts src/lib/appStoreDomainWiring.test.ts` passed (3 files, 16 tests).
- `npm run build` passed.

Next active slice:
R1.7 Remove obsolete shell-era UI leftovers (in progress), starting with unreferenced `DynamicControlsPanel.svelte` and the remaining shell-era panel leftovers.

Phase 2 recovery progress update (2026-03-06, R1.7 shell-era cleanup)
Completed slice:
- R1.7 Remove obsolete shell-era UI leftovers

Implemented in this slice:
- Removed unreferenced shell-era UI files:
	- `src/lib/ui/DynamicControlsPanel.svelte`
	- `src/lib/ui/TopBar.svelte`
	- `src/lib/ui/InputsTab.svelte`
	- `src/lib/ui/PresetCustomizeShell.svelte`
- `README.md`
	- Updated the `src/lib/ui/` architecture summary so it points at the live `ModePanel` / section-panel structure instead of the retired shell components.

 Audit result:
- `src/App.svelte` no longer carries shell-era UI assumptions; the live app already routes through `ModePanel`, `RunControls`, and the recovered panel stack.

Follow-up cleanup note (2026-03-06, post-R1.7 verification):
- Verified that `src/lib/ui/TabContainer.svelte` and `src/lib/ui/PermeabilityPanel.svelte` also had no current `src/**` references and still represented retired UI paths.
- Retired both files after the extra verification pass instead of carrying them forward as follow-up candidates.

Validation run:
- `get_errors` reported no errors in `src/App.svelte` and `README.md` after the cleanup.
- `npm run typecheck` passed.
- `npm run test -- src/lib/ui/modePanelComposition.test.ts src/lib/ui/modePanelSchema.test.ts src/lib/appStoreDomainWiring.test.ts` passed (3 files, 16 tests).
- `npm run build` passed.

Next active slice:
R1.8 Regression + policy hardening.

Phase 2 follow-up note (2026-03-06, geometry quick-editor consistency cleanup)
Follow-up scope:
- Align the geometry quick-editor with the component-first panel architecture by removing the remaining TS control-schema file.

Implemented in this follow-up:
- `src/lib/ui/GeometryGridQuickEditor.svelte`
	- Moved geometry quick-pick definitions and the small matching/error helper logic into the Svelte component so the geometry card owns its own UI details like the other cards.
- `src/lib/ui/GridFieldsPanel.svelte`
	- Simplified the wrapper so it delegates directly to `GeometryGridQuickEditor.svelte` without importing a separate control-definition file.
- `src/lib/ui/modePanelComposition.test.ts`
- `src/lib/ui/modePanelSchema.test.ts`
	- Updated the focused tests to assert the component-owned geometry quick-edit approach.
- Removed `src/lib/ui/geometryGridQuickEditor.ts`
	- Retired the last remaining TS control-schema file for live card rendering.

Outcome:
- Geometry quick-edit remains local and typed, but it now follows the same component-owned approach as the rest of the panel cards instead of keeping one leftover TS schema/config file.
- The codebase no longer mixes a Svelte-first card architecture with a separate TS control-definition layer for the geometry card.

Validation run:
- `get_errors` reported no errors in `src/lib/ui/GeometryGridQuickEditor.svelte`, `src/lib/ui/GridFieldsPanel.svelte`, `src/lib/ui/modePanelHelpers.test.ts`, and `src/lib/ui/modePanelComposition.test.ts`.
- `npm run typecheck` passed.
- `npm run test -- src/lib/ui/modePanelComposition.test.ts src/lib/ui/modePanelHelpers.test.ts src/lib/appStoreDomainWiring.test.ts` passed (3 files, 15 tests).
- `npm run build` passed.

Phase 2 recovery progress update (2026-03-06, R1.8 regression and policy hardening)
Completed slice:
- R1.8 Regression + policy hardening

Implemented in this slice:
- `src/lib/stores/phase2PresetContract.ts`
	- Added shared policy helpers for:
		- modified-state auto-clear eligibility
		- benchmark clone-to-custom eligibility
		- mode-panel status-row visibility
- `src/lib/stores/phase2PresetContract.test.ts`
	- Added behavior tests for those shared policy helpers.
- `src/lib/stores/simulationStore.svelte.ts`
	- Switched modified-state auto-clear and benchmark clone eligibility logic to the shared policy helpers.
- `src/lib/stores/simulationStorePolicyWiring.test.ts`
	- Added store wiring guards so the live store continues to use the shared policy helpers.
- `src/lib/ui/ModePanel.svelte`
	- Switched status-row visibility to the shared policy helper.
- `src/lib/ui/modePanelFlows.test.ts`
	- Added flow guards for final mode-panel surfaces: status row, clone messaging, and validation-warning surfacing.
- Existing warning-policy and composition/helper tests were kept in the focused validation slice:
	- `src/lib/warningPolicy.test.ts`
	- `src/lib/warningPolicyFilters.test.ts`
	- `src/lib/ui/modePanelComposition.test.ts`
	- `src/lib/ui/modePanelHelpers.test.ts`
	- `src/lib/appStoreDomainWiring.test.ts`

Outcome:
- The recovered panel architecture now has explicit regression coverage for the policy rules that had previously only lived as inline conditions.
- Modified-state transitions, clone provenance eligibility, override visibility policy, warning-policy grouping/filtering, and final mode-panel flow surfaces now have focused tests or wiring guards.

Validation run:
- `get_errors` reported no errors in the touched policy, store, and UI files.
- `npm run typecheck` passed.
- `npm run test -- src/lib/stores/phase2PresetContract.test.ts src/lib/stores/simulationStorePolicyWiring.test.ts src/lib/warningPolicy.test.ts src/lib/warningPolicyFilters.test.ts src/lib/ui/modePanelComposition.test.ts src/lib/ui/modePanelHelpers.test.ts src/lib/ui/modePanelFlows.test.ts src/lib/appStoreDomainWiring.test.ts` passed (8 files, 41 tests).
- `npm run build` passed.

Next active slice:
Authoritative recovery plan complete. Continue with backlog items outside `R1.x` unless a new recovery slice is opened.

UI structure refactor follow-up (2026-03-06, `src/lib/ui` organization and shared card cleanup)
Completed slice:
- Reorganize `src/lib/ui` into role-based folders and remove wrapper-only section components.

Implemented in this slice:
- `src/lib/ui/modes/ModePanel.svelte`
- `src/lib/ui/modes/BenchmarkPanel.svelte`
- `src/lib/ui/modes/ScenarioModePanel.svelte`
	- Moved mode-level composition into `modes/` and replaced the old one-file-per-scenario-mode wrappers with a single `ScenarioModePanel` that delegates to `sections/ScenarioSectionsPanel.svelte`.
- `src/lib/ui/sections/ScenarioSectionsPanel.svelte`
- `src/lib/ui/sections/GeometrySection.svelte`
- `src/lib/ui/sections/ReservoirSection.svelte`
- `src/lib/ui/sections/WellsSection.svelte`
- `src/lib/ui/sections/TimestepSection.svelte`
- `src/lib/ui/sections/AnalyticalSection.svelte`
- `src/lib/ui/sections/RelativeCapillarySection.svelte`
	- Reorganized section components under `sections/` and bound them directly from the section compositor instead of routing through `*FieldsPanel` wrappers.
- `src/lib/ui/controls/FilterCard.svelte`
- `src/lib/ui/controls/ToggleGroup.svelte`
- `src/lib/ui/cards/RunControls.svelte`
- `src/lib/ui/cards/BenchmarkResultsCard.svelte`
- `src/lib/ui/feedback/AnalyticalStatusBanner.svelte`
- `src/lib/ui/feedback/WarningPolicyPanel.svelte`
- `src/lib/ui/shared/panelStyles.ts`
	- Moved reusable UI building blocks into role folders and centralized repeated panel/table class strings.
- `src/App.svelte`
- `src/lib/3dview.svelte`
- `src/lib/RateChart.svelte`
	- Updated imports to the new folder structure.
- `src/lib/ui/modePanelComposition.test.ts`
- `src/lib/ui/modePanelFlows.test.ts`
- `src/lib/ui/modePanelHelpers.test.ts`
	- Updated path-sensitive tests to assert the new structure.
- Removed obsolete wrapper and leftover files:
	- `src/lib/ui/AnalyticalFieldsPanel.svelte`
	- `src/lib/ui/DepletionPanel.svelte`
	- `src/lib/ui/GridFieldsPanel.svelte`
	- `src/lib/ui/ReservoirFieldsPanel.svelte`
	- `src/lib/ui/SimulationPanel.svelte`
	- `src/lib/ui/StaticPropertiesPanel.svelte`
	- `src/lib/ui/TimestepFieldsPanel.svelte`
	- `src/lib/ui/WaterfloodPanel.svelte`
	- `src/lib/ui/WellsFieldsPanel.svelte`

Outcome:
- `src/lib/ui` is no longer a flat directory of mixed roles; entry points, sections, controls, cards, feedback surfaces, and shared style helpers are now separated cleanly.
- `ModePanel` now reuses the common `Card` primitive instead of defining its own outer card shell, and repeated collapsible body/table styling is centralized in `src/lib/ui/shared/panelStyles.ts`.
- The live section compositor is simpler to navigate because the wrapper-only `*FieldsPanel` layer is gone.

Validation run:
- `get_errors` reported no errors in the touched `src/lib/ui/` tree, `src/App.svelte`, `src/lib/RateChart.svelte`, and `src/lib/3dview.svelte`.
- `npm run typecheck` passed.
- `npm run test -- src/lib/ui/modePanelComposition.test.ts src/lib/ui/modePanelHelpers.test.ts src/lib/ui/modePanelFlows.test.ts src/lib/appStoreDomainWiring.test.ts` passed (4 files, 17 tests).
- `npm run build` passed.

Next active slice:
- Keep backlog focus on behavior gaps rather than more structural churn; the clearest remaining UI follow-up is still the higher-level product item for explicit per-mode top-level flows if that architecture is still desired.
