# Architecture Notes

This document tracks active architectural decisions that are still relevant. Historical implementation detail and delivered work have been moved out to `docs/DELIVERED_WORK_2026_Q1.md`.

## Current Architecture Snapshot

- `src/lib/catalog/scenarios.ts` is the primary scenario registry.
- There are 9 canonical scenario definitions under `src/lib/catalog/scenarios/`.
- `ScenarioCapabilities` is the main routing contract for analytical behavior, chart defaults, injector presence, sweep geometry, and three-phase gating.
- The custom-mode UI is denser and more structured than before, but it still sits beside the scenario system rather than on top of it.
- The benchmark layer is partially modernized but still split across legacy family-owned files and the newer scenario workflow.

## Active Architecture Priorities

### 1. Per-scenario overrides instead of mode switching

Best direction:

- Keep the user inside the selected scenario.
- Track overrides per field with reset behavior and provenance.
- Recompute analytical outputs only when the overridden parameter actually affects the active analytical method.

Why:

- The scenario system is now the clearest mental model in the app. Full custom mode should be an escape hatch, not the normal extension path.

### 2. Output-selection view model extraction

Needed outcome:

- One typed helper that resolves the active runtime or comparison payload for charts, 3D view, and analytical helpers.

Why:

- `App.svelte` still owns too much branching and too many parallel derived values.
- Runtime charts, comparison charts, and spatial output should not each infer active result state independently.

### 3. Finish the benchmark-to-scenario consolidation

Needed outcome:

- Reduce or remove the remaining dependence on `benchmarkCases.ts`, `caseCatalog.ts`, and `ReferenceExecutionCard`.
- Move shared reference-run contracts into a clearer module that is owned by the scenario architecture instead of by the older benchmark layer.

Why:

- The current split is still the largest source of conceptual duplication in the frontend.

### 4. Strengthen analytical contracts

Needed outcome:

- One analytical-method contract that governs output types, valid primary curves, x-axis behavior, disclosure metadata, and comparison behavior.

Why:

- The project already fixed one analytical-contract drift bug. The next step is to make similar drifts structurally harder to introduce.

## Architecture Constraints

- Svelte 5 runes only.
- Tailwind-first styling.
- Three.js pinned at `0.183.2`.
- Worker communication must remain structured-clone-safe.
- No unnecessary new dependencies.
