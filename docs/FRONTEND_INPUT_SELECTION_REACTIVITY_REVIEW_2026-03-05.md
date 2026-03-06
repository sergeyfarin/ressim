# Frontend Review: Input Selection, Reactivity, and Event Logic

Date: 2026-03-05
Scope: `src/App.svelte`, `src/lib/stores/simulationStore.svelte.ts`, `src/lib/ui/*`, `src/lib/catalog/caseCatalog.ts`, `src/lib/catalog/catalog.json`

Status note (2026-03-06):
- This document remains the historical review/rationale that led to the current direction.
- It is not the authoritative execution tracker anymore. Use `TODO.md` under `Authoritative Recovery Plan — Schema-Driven Composer` for active work.
- Some implementation details referenced here were superseded by later changes (`ModePanel`, pre-run removal, per-mode catalog schema), but the product rationale still stands.

## Executive Summary

The frontend architecture is functional, but state/event logic is over-coupled in the store and has a few correctness and UX defects. The biggest issues are:

1. Validation state is not wired to run controls, so invalid runs are not properly blocked in UI.
2. Custom sub-case switching appears broken due to mode key mismatch.
3. Facet auto-repair is one-pass and can leave invalid combinations after cascading rule changes.
4. Layer permeability editing can be unintentionally reset on `nz` edits.

Your idea is directionally strong: keep pre-calculated cases only for benchmark mode, run depletion/waterflood/simulation directly in WASM, and move to a combined preset + customizable input workflow with explicit analytical-eligibility rules.

Decision update from user:
- Analytical overlays should be permissive-with-warning (not strict blocking).
- Benchmark mode should allow cloning into custom mode.

## Findings (Ordered by Severity)

### 1) High: Validation gating is not passed into `RunControls`

- Evidence:
  - `RunControls` expects `hasValidationErrors` and uses it to disable actions: `src/lib/ui/RunControls.svelte:15`, `src/lib/ui/RunControls.svelte:90`, `src/lib/ui/RunControls.svelte:98`, `src/lib/ui/RunControls.svelte:113`
  - `App` does not pass `hasValidationErrors` prop: `src/App.svelte:346`, `src/App.svelte:365`
  - Store silently ignores runs on invalid input: `src/lib/stores/simulationStore.svelte.ts:711`
- Impact:
  - User can click Run/Step buttons with invalid data and get no obvious action.
  - Feels like UI glitch rather than validation feedback.
- Recommendation:
  - Pass `hasValidationErrors={sim.hasValidationErrors}` into `RunControls`.
  - On blocked run due to invalid inputs, return explicit runtime message (not silent return).

### 2) High: Custom sub-case mapping logic is effectively broken

- Evidence:
  - Mapping keys are `depletion | waterflood | simulation`: `src/lib/stores/simulationStore.svelte.ts:106`
  - Active mode values are `dep | wf | sim | benchmark`: `src/lib/catalog/caseCatalog.ts:4`
  - Lookup is direct lowercase match: `src/lib/stores/simulationStore.svelte.ts:638`
- Impact:
  - `resolveCustomSubCase(activeMode)` returns `null` for `dep/wf/sim`, so intended custom sub-case transition likely never occurs.
- Recommendation:
  - Normalize mode names via explicit map (`dep -> depletion`, etc.) before lookup.

### 3) High: Toggle auto-repair can leave invalid combinations after cascades

- Evidence:
  - In `handleToggleChange`, disabled options are computed once and fixed once: `src/lib/stores/simulationStore.svelte.ts:777`
  - Rule dependencies can cascade (`mode` changes can invalidate `geo`, then `geo` can invalidate `well`): `src/lib/catalog/catalog.json:430`, `src/lib/catalog/catalog.json:442`
- Impact:
  - State can remain internally inconsistent with rules, depending on selection order.
- Recommendation:
  - Replace one-pass repair with iterative stabilization:
    - Recompute disabled map until no selected value is disabled, or max iterations reached.

### 4) Medium: `nz` edits can wipe per-layer permeability values

- Evidence:
  - `StaticPropertiesPanel` fires `onNzOrPermModeChange` on `nz` input: `src/lib/ui/StaticPropertiesPanel.svelte:176`
  - `InputsTab` wires this callback to hard reset arrays when `permMode === "perLayer"`: `src/lib/ui/InputsTab.svelte:155`
- Impact:
  - User loses per-layer edits while changing grid layers.
- Recommendation:
  - Use resize-preserving behavior (store already has `syncLayerArraysToGrid`): `src/lib/stores/simulationStore.svelte.ts:352`
  - Keep existing values where indices still exist; append defaults only for new layers.

### 5) Medium: Pre-run loading is unconditional across non-benchmark modes

- Evidence:
  - Every toggle change calls `loadPreRunCase(newKey)`: `src/lib/stores/simulationStore.svelte.ts:796`
  - Loader always fetches `cases/prerun/${key}.json.gz`: `src/lib/stores/simulationStore.svelte.ts:896`, `src/lib/stores/simulationStore.svelte.ts:902`
- Impact:
  - Extra async/network complexity for cases that should run instantly with WASM.
  - Additional warning/status branches and continuation logic increase cognitive and code complexity.
- Recommendation:
  - Restrict pre-run loading to benchmark mode only.

### 6) Medium: Global config diff effect is broad and side-effectful

- Evidence:
  - Unscoped `$effect` in `App` runs `sim.checkConfigDiff()`: `src/App.svelte:32`
  - `checkConfigDiff` can reset model state and stop/reinit run: `src/lib/stores/simulationStore.svelte.ts:736`
- Impact:
  - Hard to reason about when edits trigger resets.
  - Large side effects tied to broad reactive reads.
- Recommendation:
  - Move to explicit change domains:
    - Structural changes (grid/wells/physics) -> mark `modelNeedsReinit`.
    - Runtime controls (`steps`, `historyInterval`) should not trigger model reset.

### 7) Low-Medium: Validation logic is duplicated

- Evidence:
  - Store has local `validateInputs`: `src/lib/stores/simulationStore.svelte.ts:275`
  - Separate exported validator duplicates same rules: `src/lib/validateInputs.ts:49`
- Impact:
  - Rule drift risk and inconsistent behavior/tests over time.
- Recommendation:
  - Use one shared validator (`validateInputs.ts`) in store and tests.

### 8) Low: Dead/no-op branch in custom sub-case function

- Evidence:
  - `foundKey` is initialized with `activeCase`, then checked `foundKey !== activeCase`: always false: `src/lib/stores/simulationStore.svelte.ts:645`, `src/lib/stores/simulationStore.svelte.ts:647`
- Impact:
  - Confusing maintenance signal.
- Recommendation:
  - Remove dead branch or implement intended lookup.

## Input Selection Design for Scientific Apps (Best Practices)

1. Separate intent levels:
- Scenario intent (depletion/waterflood/simulation/benchmark)
- Physics assumptions (gravity, capillary, relperm model)
- Numerical controls (dt, steps, stabilization)

2. Support faceted presets with explicit constraints:
- Allow fast path via curated facets.
- Always show why options are disabled (already partially implemented).
- Use deterministic constraint resolution (stable repair loop).

3. Make analytical eligibility explicit:
- Not all parameter combinations should produce analytical overlays.
- Show `Analytical: eligible / not eligible` with reasons.

4. Preserve reproducibility:
- Track all parameter overrides from base preset.
- Make it easy to export current configuration.

5. Progressive disclosure:
- Show only high-impact controls by default.
- Collapse advanced controls (well model details, compressibility, etc.).

## Your Proposal: Pre-calc Benchmarks Only

This is a good direction.

Rationale:
- Benchmarks need deterministic reference artifacts.
- Non-benchmark workflows benefit more from flexible parameter exploration.
- WASM speed makes pre-run artifacts unnecessary for regular modes.

## Product/UX Options (Pros and Cons)

## Option A: Keep Current Split UI, Simplify Logic Internally

Description:
- Keep top facets + bottom full inputs layout.
- Remove non-benchmark pre-run loading.
- Fix defects and tighten reactivity.

Pros:
- Lowest migration risk.
- Minimal UI retraining for current users.
- Fastest path to correctness.

Cons:
- Still cognitively split (select preset at top, edit details at bottom).
- Customization intent remains less obvious.

## Option B: Unified "Preset + Customize" Surface (Recommended)

Description:
- Replace strict top-vs-bottom separation.
- At the top of inputs panel:
  - Step 1: Choose mode + facets (preset composer).
  - Step 2: Show generated parameter profile.
  - Step 3: Inline `Customize` toggles advanced groups.
- Keep benchmark as dedicated branch with pre-run/benchmark artifacts.

Pros:
- Strong mental model: "choose base profile, then override".
- Reduces hidden coupling between components.
- Better fit for faceted scientific exploration.
- No migration constraints because this is a greenfield product (no legacy users/data).

Cons:
- Moderate UI rewrite.
- Requires clear interaction design and robust acceptance tests to avoid regressions during refactor.

## Option C: Strict Profiles + Full Custom as Separate Mode

Description:
- `Benchmark`, `Guided` (facet-only), and `Custom` (all parameters) as explicit modes.
- Analytical overlays enabled only in Guided/Benchmark with valid constraints.

Pros:
- Highest clarity and guardrails.
- Very robust for teaching/demo workflows.

Cons:
- Less flexible unless user switches modes.
- More mode transitions and UX state management.

## Recommended Direction

Choose Option B with benchmark-only pre-run artifacts.
Status: selected by product owner.

Why:
- Preserves power-user flexibility.
- Simplifies event/reactivity model.
- Aligns with your goal: unlimited combinations outside benchmark mode.

Refinement from product decisions:
- Keep analytical overlays available for approximate/non-ideal combinations, but show a clearly visible warning badge and rationale.
- Add a one-click `Clone to Custom` path from benchmark presets so users can start from validated references and branch into exploration.

## Analytical Compatibility Strategy

Introduce an `AnalyticalEligibility` evaluator:

- Input: current parameter state.
- Output:
  - `eligible: boolean`
  - `mode: depletion | waterflood | none`
  - `reasons: string[]`

Rules example:
- Waterflood analytical preferred when injector enabled and geometry approximates 1D displacement.
- Depletion analytical preferred when injector disabled and producer control assumptions are met.
- If assumptions are weak or violated, still show overlay as `Approximate` with a prominent warning and listed caveats.

Suggested status model:
- `reference`: assumptions match benchmark-grade conditions.
- `approximate`: overlay shown with visible warning banner + tooltip reasons.
- `off`: user manually disables analytical overlay.

## Refactor Plan (Phased)

### Phase 0: Correctness Hotfixes (short)

1. Wire `hasValidationErrors` into `RunControls` and show explicit blocked-run feedback.
2. Fix custom sub-case mode key mapping and remove dead branch.
3. Make facet auto-fix iterative until stable.
4. Preserve per-layer arrays on `nz` edits.

### Phase 1: State Simplification (short-medium)

1. Separate state domains:
- `scenarioSelection` (mode + facets + benchmark id)
- `parameterState` (actual numeric/boolean values)
- `runtimeState` (worker/history/progress)

2. Move validation to single source (`validateInputs.ts`).
3. Replace broad diff checking with explicit dirty flags by domain.

### Phase 2: UX Consolidation (medium)

1. Build unified preset+custom panel.
2. Add per-group reset-to-preset and "show changed fields".
3. Display analytical status panel (`reference` vs `approximate`) with prominent warnings for approximate mode.
4. Add benchmark `Clone to Custom` action preserving provenance (source benchmark id).

### Phase 3: Benchmark Policy (medium)

1. Restrict pre-run loading to benchmark mode.
2. Keep benchmark artifacts deterministic and versioned.
3. For non-benchmark, always run direct WASM.
4. Support cloning benchmark presets into custom editable runs.

## Suggested Acceptance Criteria

1. No silent no-op on Run/Step actions when invalid.
2. No disabled facet remains selected after any toggle change.
3. Editing `nz` never destroys per-layer values unexpectedly.
4. Non-benchmark selection never triggers pre-run fetch.
5. Analytical overlay remains available for approximate cases with a clearly visible warning and explicit assumptions/caveats.
6. A single validation implementation is used in runtime + tests.
7. Benchmark presets can be cloned into custom mode with traceable source metadata.

## Notes

- Existing faceted catalog (`src/lib/catalog/catalog.json`) is a good base and should be retained.
- Benchmark entries should remain curated and immutable for reproducibility.
- Keep the current disabled-reason UX; it is already useful for scientific workflows.

## 2026-03-06 Follow-Up Decisions

1. Use a typed schema-driven UI composition model.
- Layout, labels, control types, option lists, formatting metadata, and simple parameter patches may come from JSON/TS config.
- Constraint logic, physical rules, simulator payload transforms, and cross-field behavior must stay in TypeScript.

2. Keep the system permissive by severity, not permissive by silence.
- Blocking validation errors should disable run/init and show inline field errors plus explicit runtime messaging.
- Contradictory or non-physical-but-editable states should remain visible/editable but produce prominent warnings.
- Analytical/reference-model caveats should stay permissive with clearly visible warning badges/banner reasons.

3. Support quick-select plus custom-entry controls.
- Example: a grid-density control may offer quick picks (`12`, `24`, `48`) plus a `Custom` affordance that reveals a typed numeric input.
- Example: advanced physics groups such as Corey/SCAL inputs may expose a compact preset choice with an expandable custom parameter section.

4. Migrate the current unified panel incrementally.
- First restore truthful preset/customize semantics in the live panel.
- Then move one section (`Geometry + Grid`) to schema-driven rendering before replacing the rest.
