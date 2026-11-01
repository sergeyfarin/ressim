# SPE5 Case 1 payloads (#52)

| File | What it is |
|---|---|
| `case.json` | The exact `CompositionalCaseConfig` the withheld `comp_spe5_wag` scenario sends. Exported from the TS definition and checked against it by the scenario test. |
| `stall_192.json` | The accepted checkpoint at month 192 (start of the water year in which the Newton stall occurs). Reproducer seed for [#93](https://github.com/sergeyfarin/ressim/issues/93); not a validated reference. Regenerate with `SPE5_CHECKPOINT_OUT=opm/compositional/spe5/stall_192.json` on the 16-year replay test. |

The reference fixture and its manifest will live here when the JutulDarcy comparison lands. There
is deliberately no `reference.json` yet: the case has no external result to quote.
