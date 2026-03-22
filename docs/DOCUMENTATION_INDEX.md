# Documentation Index

Use this file to decide which documents are authoritative, which are active working notes, and which are historical snapshots.

## Authoritative References

| Document | Use it for |
|----------|------------|
| `README.md` | Product overview, current feature state, quick start, and doc map |
| `ROADMAP.md` | Future-facing roadmap and priority order |
| `TODO.md` | Active execution tracker only |
| `docs/ARCHITECTURE_NOTES.md` | Current architecture direction and unresolved design decisions |
| `docs/DELIVERED_WORK_2026_Q1.md` | Archived delivered work moved out of TODO |
| `docs/BENCHMARK_MODE_GUIDE.md` | Current benchmark workflow semantics and comparison behavior |
| `docs/P4_TWO_PHASE_BENCHMARKS.md` | Buckley-Leverett benchmark methodology, tolerances, and current results |
| `docs/THREE_PHASE_IMPLEMENTATION_NOTES.md` | Three-phase implementation details, conventions, and remaining validation gaps |
| `docs/UNIT_SYSTEM.md` | Unit conventions, equations, and solver / PVT notes |
| `docs/UNIT_REFERENCE.md` | Quick unit lookup card |
| `docs/TRANSMISSIBILITY_FACTOR.md` | Derivation of the transmissibility conversion factor |

## Current Repo-Level Facts

- `src/lib/catalog/scenarios.ts` is the primary scenario registry.
- There are 9 canonical scenarios under `src/lib/catalog/scenarios/`.
- `ScenarioPicker.svelte` is the main scenario-selection surface.
- Legacy benchmark-family files still exist and remain load-bearing in parts of the UI and chart stack.
- All simulations execute directly in browser-side WASM. There is no prerun artifact pipeline.
- Black-oil mode is implemented and exposed in the UI, but its validation backlog is still open.
- Three-phase mode is implemented, but remains experimental because validation depth still trails the implementation.

## Historical Snapshots

These files are useful context, but they are not live specs.

| Document | Status |
|----------|--------|
| `docs/FRONTEND_UI_AUDIT_2026-03-07.md` | Historical frontend audit that fed later refactoring work |
| `docs/IMPLEMENTATION_REVIEW_2026-03-19.md` | Historical implementation review snapshot; some findings have since been closed |

## Maintenance Rule

If a document stops describing the current implementation or current plan, either update it immediately or demote it to historical status. Do not leave half-current working documents in the authoritative set.
