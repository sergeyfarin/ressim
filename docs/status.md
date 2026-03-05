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
