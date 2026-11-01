# SPE5 Case 1 payloads (#52)

| File | What it is |
|---|---|
| `case.json` | The exact `CompositionalCaseConfig` the withheld `comp_spe5_wag` scenario sends. Exported from the TS definition and checked against it by the scenario test. |
| `stall_192.json` | The accepted checkpoint at month 192 (start of the water year in which the #93 Newton stall occurred). Seed for the month-192 → 201 regression; not a validated reference. Regenerate with `SPE5_CHECKPOINT_OUT=opm/compositional/spe5/stall_192.json` on the 16-year replay test. |
| `stall_call_058.json` | The accepted state at 6114.808 d, immediately before the failing #93 call. Fast regression state for the near-critical stability fix; captured from the pre-fix replay and deliberately not regenerated. Not a validated reference. |

The reference fixture and its manifest will live here when the JutulDarcy comparison lands. There
is deliberately no `reference.json` yet: the case has no external result to quote.
