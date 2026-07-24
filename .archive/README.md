# Archive

Code and data moved out of the active tree but kept for reference / possible
re-implementation, rather than deleted outright. Everything here is git-tracked
and reversible with `git mv`.

## `docs/` — superseded documentation (archived 2026-07-24)

Documentation cleanup: closed experiments, superseded plans, and dated
snapshots moved out of `docs/` so the active set only contains authoritative or
in-progress material (see `docs/DOCUMENTATION_INDEX.md`). Verdicts for every
archived FIM experiment remain summarized in `docs/FIM_EXPERIMENT_REGISTRY.md`;
these files are provenance, not live specs.

- **Closed FIM experiment plans** — `FIM_BUNDLE_N_DESIGN.md`,
  `FIM_BUNDLE_P_PLAN.md`, `FIM_BUNDLE_W_PLAN.md`, `FIM_BUNDLE_X_PLAN.md`,
  `FIM_DIAG_003_PLAN.md` (all evaluated/closed; registry rows retained).
- **Superseded March–April design / audit / investigation docs** —
  `FIM_MIGRATION_PLAN.md`, `FIM_PHASE2_EXECUTION_PLAN.md`,
  `FIM_CPR_IMPROVEMENT_PLAN.md`, `FIM_LINEAR_SOLVER_AUDIT.md`,
  `FIM_BYPASS_AUDIT.md`, `FIM_JACOBIAN_REUSE_INVESTIGATION.md`,
  `FIM_SLICE_A_EXTRAPOLATION.md`, `FIM_CHOP_WIDEN_EXPERIMENT.md`,
  `FIM_UPWINDING_FRONT_STABILITY.md`, `FIM_WIDE_ANGLE_ANALYSIS.md`,
  `FIM_CONVERGENCE_IMPROVEMENTS.md`, `FIM_CLEANUP_PLAN.md`.
- **Superseded test-planning / coverage snapshots** —
  `FIM_PHYSICS_TEST_PLAN.md`, `FIM_TEST_CLASSIFICATION.md`,
  `FIM_TEST_COMPLETENESS_REVIEW.md`, `SOLVER_TEST_COVERAGE_PLAN.md`,
  `SOLVER_TEST_OWNERSHIP_INVENTORY.md`, `SOLVER_DIAGNOSTIC_COVERAGE_MATRIX.md`,
  `SOLVER_LAYOUT_REFACTOR_PLAN.md`.
- **Pre-existing history archives** (consolidated here) —
  `FIM_CONVERGENCE_ARCHIVE_2026-03_to_2026-04-06.md`,
  `FIM_CONVERGENCE_ARCHIVE_2026-04-08_to_2026-07-03.md`,
  `FIM_HISTORY_2026-03.md`.
- **Dated snapshots / reviews** — `FRONTEND_UI_AUDIT_2026-03-07.md`,
  `IMPLEMENTATION_REVIEW_2026-03-19.md`, `DELIVERED_WORK_2026_Q1.md`.

Also archived here: `PLAN.md` (historical scenario-first rewrite plan),
`docs/REFACTOR_PLAN.md` (historical refactor plan), and
`CODEX_FIM_DIALOGUE_03.07.2026.md` (historical design dialogue).

### `docs/TODO_HISTORY_2026-07-24.md` — full prior TODO snapshot

The 1,474-line `TODO.md` as it stood on 2026-07-24, before it was pruned to an
open-items-only tracker. Preserves every completed Wave 0–4 entry and the full
FIM experiment narrative (also summarized in `docs/FIM_EXPERIMENT_REGISTRY.md`).
The live `TODO.md` now carries only open items, reprioritized so user-facing
work leads and FIM convergence is a parked maintenance track.

To resurrect any of these, `git mv` it back to its original location and re-add
its row to `docs/DOCUMENTATION_INDEX.md`.

## `src/lib/catalog/custom-mode/`

Archived 2026-07-20 when Custom Mode was removed from the production UI. The
scenario picker is now the only live case-selection path, and all production
case definitions live in `src/lib/catalog/scenarios/` (one module per
scenario). This archive preserves the former JSON facet catalog and its named
starter presets for a future, separately designed custom-workflow effort.

The small live `caseCatalog.ts` compatibility surface intentionally contains no
case definitions or selectable presets.

## `src/lib/catalog/benchmarkCases.ts` + `src/lib/catalog/benchmark-case-data/*.json`

Archived 2026-07 (frontend execution plan, Wave 3 W3.3) as part of retiring
the legacy "benchmark family" system: 5 cases (`bl_case_a_refined`,
`bl_case_b_refined`, `dietz_sq_center`, `dietz_sq_corner`, `fetkovich_exp`)
that predate the scenario-first architecture (`src/lib/catalog/scenarios.ts`).

**Confirmed unreachable from the live UI before archiving** (verified three
ways): the `'benchmark'` `CaseMode` value is never used as an actual runtime
mode anywhere; `ReferenceExecutionCard.svelte` — the only component built to
display this system — is imported by zero other Svelte files, and an
existing architecture test (`src/lib/appStoreDomainWiring.test.ts`) already
asserted it must not appear in `App.svelte`; `ScenarioPicker.svelte` (the
actual picker) is driven entirely by `scenarios.ts` and has no case-library
or benchmark awareness. This is not a coverage gap in a *live* feature.

The live `src/lib/catalog/benchmarkCases.ts` was replaced with a stub that
preserves the exact same exported names/types (re-exporting the pure types
from `src/lib/scenario/referenceTypes.ts` unchanged) but returns empty
data (`[]`/`null`) instead of the 5 real families. This means every existing
consumer (`runtimeStore.svelte.ts`, `navigationStore.svelte.ts`,
`caseLibrary.ts`, `ReferenceExecutionCard.svelte`) needed **zero code
changes** — they already null-check defensively, and nothing in the live UI
ever populated these paths with a real value regardless.

To resurrect: copy `benchmarkCases.ts` and `benchmark-case-data/*.json` back
to their original locations under `src/lib/catalog/`, replacing the stub.
Some of these cases may be better re-implemented as proper entries in
`src/lib/catalog/scenarios/` instead (see `docs/CASE_LIBRARY_ROADMAP.md`) —
check before restoring verbatim.

### Test coverage flagged as a gap, not silently dropped

Four test files used the archived data as fixtures to exercise still-live
code (`benchmarkRunModel.ts`'s generic run-spec functions,
`referenceChartConfig.ts`, `buildChartData.ts`). The specific test cases that
depended on archived fixtures were marked `.skip` with a comment pointing
here, rather than deleted or silently left broken — see
`docs/TODO.md` for the tracked follow-up to rewrite them against
scenario-based fixtures.
