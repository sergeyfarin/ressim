# OPM Reference Decks

These decks are Phase-0 validation fixtures for the OPM-compatible FIM
reimplementation plan. They are tracked inputs, not generated output.

Run them through `scripts/opm-ressim-compare.sh`; the harness copies each deck
into the selected comparison output directory before invoking Flow so OPM
restart, summary, and report files do not dirty this tree.

The decks intentionally mirror the current `scripts/fim-wasm-diagnostic.mjs`
presets as closely as Eclipse/Flow syntax allows. `CASE.DATA` is the same-dt
deck, `CASE_DT4.DATA` is the dt/4 refinement deck, and `CASE_DT16.DATA` is the
dt/16 refinement deck.

Initial Flow parse/run validation was performed for:

- `water-medium-step1`
- `water-medium-6step`
- `gas-rate-10x10x3`

Run all tracked variants for a case with:

```bash
scripts/opm-ressim-compare.sh --opm-only --case water-medium-step1 --opm-variant all --flow-bin /usr/bin/flow
```
