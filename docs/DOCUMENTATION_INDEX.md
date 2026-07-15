# Documentation Index

Use this file to decide which documents are authoritative, which are active working notes, and which are historical snapshots.

## Authoritative References

| Document | Use it for |
|----------|------------|
| `README.md` | Product overview, current feature state, quick start, and doc map |
| `ROADMAP.md` | Future-facing roadmap and priority order |
| `TODO.md` | Active execution tracker only |
| `docs/ARCHITECTURE_NOTES.md` | Current architecture direction and unresolved design decisions |
| `docs/FIM_DEFERRED_BACKLOG.md` | Current product boundary for FIM and the deferred solver backlog |
| `docs/FIM_STATUS.md` | Current consolidated FIM implementation state, blockers, validation entry points, and canonical source map |
| `docs/FIM_EXPERIMENT_REGISTRY.md` | Searchable anti-repeat ledger of FIM convergence experiments, verdicts, retry conditions, and source-doc links |
| `docs/FIM_CONVERGENCE_WORKLOG.md` | Active FIM investigation log for current-head traces and temporary hypotheses (Phase 9 onward) |
| `docs/FIM_CONVERGENCE_ARCHIVE_2026-04-08_to_2026-07-03.md` | Archived worklog: shelf investigations, AD-assembler cutover, Phases 5-8, Hypothesis C |
| `docs/FIM_OPM_ALIGNMENT_STRATEGY_2026-04-26.md` | The 95%-track-OPM policy decision and Bundle A/B/C sequencing, with 2026-07-05 status addendum |
| `docs/FIM_OPM_GAP_ANALYSIS_SPE1.md` | Six-item FIM-vs-OPM Newton-efficiency gap decomposition, with 2026-07-05 triage (4 of 6 closed) |
| `docs/FIM_OPM_CONVERGENCE_EXECUTION_PLAN.md` | Active decision-frontier plan: Y2b2a backend-neutral oracle repair, corrected raw-state replay, dependency-aware OPM lifecycle gates, promotion matrix, and prescriptive simple-model handoff |
| `docs/FIM_Y2B3_PRIMARY_VARIABLE_LIFECYCLE_DESIGN.md` | Deck-scoped OPM `Sg`/`Rs` lifecycle map, ResSim fixed-layout dependency contract, empty-column invariant, and prescriptive Y2b3a-c implementation gates |
| `docs/FIM_Y2D6_FLOW_LINEAR_LIFECYCLE_DESIGN.md` | Source-pinned Flow 2026.04 BiCGSTAB/true-IMPES/CPRW/paroverilu0/one-loop-AMG lifecycle, well-operator split, coupled capture gates, and IMPES applicability audit |
| `docs/FIM_BUNDLE_N_DESIGN.md` | Bundle N (OPM nonlinear layer): design, §9 verified OPM formulas, §5.1 failed end-metric evaluation, §10 retrospective/disposition — parked behind the `OpmAligned` flag |
| `docs/FIM_BUNDLE_P_PLAN.md` | Bundle P (CPR setup reuse): REFUTED at P0 2026-07-10 (no failure-free reuse interval); P0 measurement tooling kept; superseded on the cost axis by `FIM-LINEAR-011` |
| `docs/FIM_BUNDLE_W_PLAN.md` | Bundle W (nested well solve): EVALUATED 2026-07-11, mechanism validated (standoff fixed), NOT promoted — heavy-case gate failed on a newly-exposed second gap; kept inert behind `nested_well_solve` |
| `docs/FIM_DIAG_003_PLAN.md` | FIM-DIAG-003 (MB plateau under `OpmAligned`): CLOSED 2026-07-12, D0-D5 complete — H1 confirmed three ways, H2/H3 refuted; mechanism located, fixed by Bundle X |
| `docs/FIM_BUNDLE_X_PLAN.md` | Bundle X (producer-fraction fidelity): CLOSED/PROMOTED unconditional 2026-07-12 — 3x3-neighborhood producer fraction replaced by OPM's single-cell formula; heavy case 18,015→16 (`OpmAligned`+nested) / 52→25 (Legacy default) |
| `docs/FIM_OPM_PARITY_PLAN.md` | Bundle Y evidence record and original Y0-Y4 roadmap; current execution order is owned by `FIM_OPM_CONVERGENCE_EXECUTION_PLAN.md` |
| `docs/FIM_MIGRATION_PLAN.md` | File-by-file FIM cutover checklist, target solver architecture, and proposed Rust APIs |
| `docs/FIM_CLEANUP_PLAN.md` | FIM doc/test/diagnostic cleanup sequence and ownership boundaries |
| `docs/OPM_FLOW_MINIMAL_MAPPING.md` | Minimal OPM Flow to ResSim solver mapping and the concrete CPRW-first implementation plan |
| `docs/DELIVERED_WORK_2026_Q1.md` | Archived delivered work moved out of TODO |
| `docs/BENCHMARK_MODE_GUIDE.md` | Current benchmark workflow semantics and comparison behavior |
| `docs/P4_TWO_PHASE_BENCHMARKS.md` | Buckley-Leverett benchmark methodology, tolerances, and current results |
| `docs/THREE_PHASE_IMPLEMENTATION_NOTES.md` | Three-phase implementation details, conventions, and remaining validation gaps |
| `docs/UNIT_SYSTEM.md` | Unit conventions, equations, and solver / PVT notes |
| `docs/UNIT_REFERENCE.md` | Quick unit lookup card |
| `docs/TRANSMISSIBILITY_FACTOR.md` | Derivation of the transmissibility conversion factor |
| `docs/SCENARIO_TERMINATION_POLICY.md` | Early-stop policy syntax, supported conditions, and runtime behavior |
| `docs/COMPARISON_TOOLBOX_REVIEW_2026-07-01.md` | 2026-07 findings and forward plan for comparison architecture, OPM Flow integration, and scenario coverage |
| `docs/CASE_LIBRARY_ROADMAP.md` | Sourcing map for new scenarios/cases: SPE benchmarks, public field datasets, textbook cases, and selection criteria |
| `.claude/skills/README.md` | Workflow skill library index (validation, engine changes, FIM debugging, frontend, scenarios, OPM pipeline) and how to use it with different agents |

## Current Repo-Level Facts

- `src/lib/catalog/scenarios.ts` is the primary scenario registry.
- There are 10 canonical scenarios under `src/lib/catalog/scenarios/`.
- `ScenarioPicker.svelte` is the main scenario-selection surface.
- Legacy benchmark-family files still exist and remain load-bearing in parts of the UI and chart stack.
- Public simulations execute directly in browser-side WASM through the IMPES path. Offline OPM Flow artifacts are precomputed reference data, not live browser simulation.
- Black-oil mode is implemented and exposed in the UI. SPE1 benchmark scenario is in place with published reference overlays and OPM Flow artifact hooks; quantitative acceptance criteria remain deferred.
- Three-phase mode is implemented, but remains experimental because validation depth still trails the implementation.

## FIM Document Ownership

- `docs/FIM_STATUS.md`: current FIM truth, blockers, validation surface, and canonical links.
- `docs/FIM_EXPERIMENT_REGISTRY.md`: short searchable verdicts for attempted or proposed convergence levers; check this before new FIM tuning.
- `docs/FIM_CONVERGENCE_WORKLOG.md`: active current-head traces, detailed measurements, and temporary reasoning while an issue is live.
- `docs/FIM_MIGRATION_PLAN.md`: target architecture and migration checklist, not a live debugging diary.
- `TODO.md`: short active tasks only; do not add long FIM experiment narratives there.

## Historical Snapshots

These files are useful context, but they are not live specs.

| Document | Status |
|----------|--------|
| `docs/FRONTEND_UI_AUDIT_2026-03-07.md` | Historical frontend audit that fed later refactoring work |
| `docs/IMPLEMENTATION_REVIEW_2026-03-19.md` | Historical implementation review snapshot; some findings have since been closed |
| `PLAN.md` | Historical scenario-first rewrite plan; superseded by work already landed (see `docs/COMPARISON_TOOLBOX_REVIEW_2026-07-01.md` §2.1). Left in place pending author decision to archive or delete. |
| `docs/REFACTOR_PLAN.md` | Historical refactor plan; most phases (0–7) are done and the file's own "last updated" date and "Phase 8 ← NOW" marker are stale (see `docs/COMPARISON_TOOLBOX_REVIEW_2026-07-01.md` §2.1) |

## Maintenance Rule

If a document stops describing the current implementation or current plan, either update it immediately or demote it to historical status. Do not leave half-current working documents in the authoritative set.
