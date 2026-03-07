# Documentation Index

Use this file to decide which documents are authoritative and which are preserved only as historical context.

## Current Authoritative Docs

| Document | Use it for |
|----------|------------|
| `README.md` | Product overview, quick start, project layout, current feature summary |
| `TODO.md` | Active roadmap and the authoritative execution tracker for current frontend recovery work |
| `docs/FRONTEND_UI_AUDIT_2026-03-07.md` | Current frontend/product audit covering workflow, warnings, charts, visualization, labels, and layout priorities |
| `docs/status.md` | Current snapshot plus chronological execution log for completed slices and validations |
| `docs/BENCHMARK_MODE_GUIDE.md` | Current benchmark-family registry, execution workflow, reference policy, and chart behavior |
| `docs/P4_TWO_PHASE_BENCHMARKS.md` | Two-phase Buckley-Leverett benchmark methodology and accepted tolerances |
| `docs/UNIT_SYSTEM.md` | Unit conventions used by the current implementation |
| `docs/UNIT_REFERENCE.md` | Quick unit lookup card |
| `docs/TRANSMISSIBILITY_FACTOR.md` | Implementation note for the transmissibility conversion factor used in the Rust solver |
| `docs/PHASE2_PRESET_CUSTOMIZE_CONTRACT.md` | Store-facing preset/customize contract that the current UI continues to consume |

## Historical / Archival Docs

These files are kept because they explain how the current direction was chosen, but they should not be treated as the active implementation plan.

| Document | Status |
|----------|--------|
| `docs/FRONTEND_INPUT_SELECTION_REACTIVITY_REVIEW_2026-03-05.md` | Archived review that informed the current Option B and mode-specific-panels direction |
| `docs/PHYSICS_REVIEW.md` | Archived pre-P4 audit note; several findings were resolved or superseded by later implementation work |

## Current Repo-Level Facts

- All scenarios now initialize and run directly in browser-side WASM.
- There is no `bench:export` script, generated `benchmark-results.json` pipeline, or tracked `public/cases/prerun/` artifact tree.
- The current frontend direction is tracked in `docs/FRONTEND_UI_AUDIT_2026-03-07.md` and the prioritized execution list in `TODO.md`.
- Benchmark mode behavior is documented in `docs/BENCHMARK_MODE_GUIDE.md`, while Buckley-Leverett acceptance methodology remains in `docs/P4_TWO_PHASE_BENCHMARKS.md`.

## Maintenance Rule

When a document stops describing the current implementation, either:

1. update it to match the code, or
2. mark it clearly as historical and link to the authoritative replacement.