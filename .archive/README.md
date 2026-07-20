# Archive

Code and data moved out of the active tree but kept for reference / possible
re-implementation, rather than deleted outright.

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
