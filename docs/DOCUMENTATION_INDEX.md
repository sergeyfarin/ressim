# Documentation Index

Use this file to decide which documents describe the current repository state.

## Authoritative References

| Document | Use it for |
|----------|------------|
| `README.md` | Product overview, quick start, project layout, feature summary, physics status |
| `TODO.md` | Prioritized work items and product roadmap (F4–F9) |
| `REFACTOR.md` | Architecture working document — Phase 1 (case-library simplification) mostly complete, Step 7 file deletions pending; Phase 2 (scenario/sensitivity redesign S1) design spec |
| `docs/BENCHMARK_MODE_GUIDE.md` | Benchmark scenario reference guidance, sensitivity policy, and chart defaults (**note**: references pre-simplification architecture; update pending after S1 lands) |
| `docs/IMPLEMENTATION_REVIEW_2026-03-19.md` | Verified scientific gaps, stale assumptions, and recommended follow-up work |
| `docs/P4_TWO_PHASE_BENCHMARKS.md` | Buckley-Leverett benchmark methodology, acceptance tolerances, and results |
| `docs/THREE_PHASE_IMPLEMENTATION_NOTES.md` | Three-phase (Stone II) architecture decisions and parameter reference |
| `docs/UNIT_SYSTEM.md` | Unit conventions, IMPES equations, and WASM API reference |
| `docs/UNIT_REFERENCE.md` | Quick unit lookup card |
| `docs/TRANSMISSIBILITY_FACTOR.md` | Derivation of the `8.527×10⁻³` transmissibility constant |

## Current Repo-Level Facts

- **Scenario model**: `src/lib/catalog/scenarios.ts` is the single source of truth for all predefined scenarios. S1 is complete; the current surface exposes 8 canonical scenarios across Waterflood / Sweep / Depletion / Gas, each with zero or more sensitivity dimensions.
- **Input UI**: `src/lib/ui/modes/ScenarioPicker.svelte` is the primary input surface. Domain tabs and sensitivity-dimension selection are live.
- **Sweep efficiency**: `src/lib/analytical/sweepEfficiency.ts` + `SweepEfficiencyChart.svelte` implement Craig areal, Dykstra-Parsons vertical, and combined volumetric sweep analytical models.
- **Legacy catalog files** (`caseCatalog.ts`, `benchmarkCases.ts`, `caseLibrary.ts`, etc.) still exist with active dependencies — removal blocked on Step 7 (see REFACTOR.md Phase 1).
- **All scenarios** initialize and run directly in browser-side WASM. No pre-run artifact pipeline.
- **Three-phase** (oil/water/gas) simulation is implemented via Stone II relative permeability (`threePhaseModeEnabled` flag). Experimental — no analytical reference solution.
- **Review note**: see `docs/IMPLEMENTATION_REVIEW_2026-03-19.md` for verified analytical-contract gaps and scientific documentation updates.
- **Historical rationale** and completed execution history live in git history rather than tracked docs.

## Historical Documents

These files are kept for reference but are no longer current:

| Document | Status |
|----------|--------|
| `docs/FRONTEND_UI_AUDIT_2026-03-07.md` | Product audit that generated the F1–F9 workstream; superseded by `TODO.md` as live tracker |

## Maintenance Rule

When a document stops describing the current implementation, either update it to match the code, or remove it. Rely on git history for completed work — do not keep zombie docs in the active reference set.
