# Documentation Index

Use this file to decide which documents describe the current repository state.

## Current Authoritative Docs

| Document | Use it for |
|----------|------------|
| `README.md` | Product overview, quick start, project layout, current feature summary |
| `TODO.md` | Active roadmap and the authoritative execution tracker for current frontend recovery work |
| `docs/FRONTEND_UI_AUDIT_2026-03-07.md` | Current frontend/product audit covering workflow, warnings, charts, visualization, labels, and layout priorities |
| `docs/BENCHMARK_MODE_GUIDE.md` | Current benchmark-family registry, family-owned reference workflow, reference guidance, and chart behavior |
| `docs/P4_TWO_PHASE_BENCHMARKS.md` | Two-phase Buckley-Leverett benchmark methodology and accepted tolerances |
| `docs/UNIT_SYSTEM.md` | Unit conventions used by the current implementation |
| `docs/UNIT_REFERENCE.md` | Quick unit lookup card |
| `docs/TRANSMISSIBILITY_FACTOR.md` | Implementation note for the transmissibility conversion factor used in the Rust solver |
| `docs/PHASE2_PRESET_CUSTOMIZE_CONTRACT.md` | Store-facing preset/customize contract that the current UI continues to consume |

## Current Repo-Level Facts

- All scenarios now initialize and run directly in browser-side WASM.
- There is no `bench:export` script, generated `benchmark-results.json` pipeline, or tracked `public/cases/prerun/` artifact tree.
- The current frontend direction is tracked in `docs/FRONTEND_UI_AUDIT_2026-03-07.md` and the prioritized execution list in `TODO.md`.
- Family-owned benchmark/reference workflow behavior is documented in `docs/BENCHMARK_MODE_GUIDE.md`, while Buckley-Leverett acceptance methodology remains in `docs/P4_TWO_PHASE_BENCHMARKS.md`.
- Historical rationale and completed execution history now live in git history rather than in tracked archival docs.

## Maintenance Rule

When a document stops describing the current implementation, either:

1. update it to match the code, or
2. remove it and rely on git history unless it is still needed as an active reference.